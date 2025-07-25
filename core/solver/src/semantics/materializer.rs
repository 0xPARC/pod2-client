use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashSet},
    hash::{Hash as StdHash, Hasher},
    sync::Arc,
};

use itertools::Itertools;
use pod2::middleware::{
    self, AnchoredKey, CustomPredicateRef, Hash, NativePredicate, PodId, Predicate,
    StatementTmplArg, TypedValue, Value, ValueRef, SELF,
};

use crate::{
    db::FactDB,
    engine::semi_naive::{Bindings, Fact, FactSource, Relation},
    error::SolverError,
    semantics::predicates::PredicateHandler,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MaterializeKey {
    pub predicate: Predicate,
    pub adornment: Vec<bool>,
    pub bound_args_hash: u64,
}

impl MaterializeKey {
    fn from(pred: &Predicate, tmpl_args: &[StatementTmplArg], bindings: &Bindings) -> Self {
        let (mut adorn, mut hasher) = (Vec::new(), DefaultHasher::new());
        for arg in tmpl_args {
            match arg {
                StatementTmplArg::Literal(v) => {
                    adorn.push(true);
                    v.raw().hash(&mut hasher);
                }
                StatementTmplArg::Wildcard(w) => {
                    if let Some(b) = bindings.get(w) {
                        adorn.push(true);
                        b.raw().hash(&mut hasher);
                    } else {
                        adorn.push(false);
                    }
                }
                StatementTmplArg::AnchoredKey(pod_wc, key) => {
                    if let Some(b) = bindings.get(pod_wc) {
                        adorn.push(true);
                        b.raw().hash(&mut hasher);
                    } else {
                        adorn.push(false);
                    }
                    key.raw().hash(&mut hasher);
                }
                StatementTmplArg::None => adorn.push(false),
            }
        }
        Self {
            predicate: pred.clone(),
            adornment: adorn,
            bound_args_hash: hasher.finish(),
        }
    }
}

/// The materializer is responsible for materializing statements from the database.
///
/// Given a statement template and a set of bindings, the materializer will attempt
/// to find any valid statements compatible with those bindings, with the caveat that
/// the bindings must typically provide enough information to find relevant statements.
///
/// For example, Equal(?a, ?b) where ?a and ?b are free variables is compatible with
/// *any* Equal statement. As such, we will not materialize any statements in response
/// to this query.
///
/// However, Equal(?a["foo"], ?b["bar"]), where ?a and ?b are free variables, is
/// constrained by the key part, and so in this case we would materialize and Equal
/// statement where ?a["foo"] = ?b["bar"].
///
/// Predicate-specific handlers are responsible for determining whether a statement
/// is valid, and for deducing the values of free variables.
pub struct Materializer {
    pub db: Arc<FactDB>,
    materialised_keys: RefCell<HashSet<MaterializeKey>>,
}

impl<'a> Materializer {
    pub fn new(db: Arc<FactDB>) -> Self {
        Self {
            db: Arc::clone(&db),
            materialised_keys: RefCell::new(HashSet::new()),
        }
    }

    pub fn value_ref_to_value(&self, vr: &ValueRef) -> Option<Value> {
        self.db.value_ref_to_value(vr)
    }

    fn resolve_term(&self, arg: &StatementTmplArg, bindings: &Bindings) -> Option<Value> {
        match arg {
            StatementTmplArg::Literal(v) => Some(v.clone()),
            StatementTmplArg::Wildcard(w) => {
                let binding = bindings.get(w);
                binding.cloned()
            }
            StatementTmplArg::AnchoredKey(pod_wc, key) => {
                let binding = bindings.get(pod_wc);
                if let Some(v) = binding {
                    if let TypedValue::PodId(pod_id) = v.typed() {
                        let ak = middleware::AnchoredKey::new(*pod_id, key.clone());
                        self.db.get_value_by_anchored_key(&ak).cloned()
                    } else if let TypedValue::Raw(raw) = v.typed() {
                        let pod_id = PodId(Hash(raw.0));
                        let ak = middleware::AnchoredKey::new(pod_id, key.clone());
                        self.db.get_value_by_anchored_key(&ak).cloned()
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            StatementTmplArg::None => None,
        }
    }

    /// Provides a generic way to iterate over all known facts for a custom
    /// predicate, with optional bindings for each argument.
    fn iter_custom_statements(
        &'a self,
        cpr: &'a CustomPredicateRef,
        binding_vector: &'a [Option<Value>],
    ) -> impl Iterator<Item = (Vec<Value>, FactSource)> + 'a {
        let all_statements: Vec<_> = self
            .db
            .statement_index
            .custom
            .iter()
            .filter(move |((batch_id, pred_idx, _), _)| {
                *batch_id == cpr.batch.id() && *pred_idx == cpr.index
            })
            .map(|((_, _, values), _)| values.clone())
            .collect();

        // Log when we have statements for the predicate but they don't match bindings
        if !all_statements.is_empty() {
            let matching_statements: Vec<_> = all_statements
                .iter()
                .filter(|values| {
                    binding_vector
                        .iter()
                        .zip(values.iter())
                        .all(|(filter, value)| filter.as_ref().is_none_or(|f| f == value))
                })
                .collect();

            if matching_statements.is_empty() {
                log::trace!(
                    "{} - found {} statements but none match bindings",
                    crate::pretty_print::PrettyDatabaseQuery {
                        batch_id: &cpr.batch.id(),
                        pred_idx: cpr.index,
                        binding_vector,
                    },
                    all_statements.len()
                );
            }
        }

        all_statements
            .into_iter()
            .filter(move |values| {
                binding_vector
                    .iter()
                    .zip(values.iter())
                    .all(|(filter, value)| filter.as_ref().is_none_or(|f| f == value))
            })
            .map(|vals| (vals, FactSource::Copy))
    }

    /// For a given template argument and binding, returns a list of possible values.
    fn column_choices(
        &self,
        arg_tmpl: &StatementTmplArg,
        bindings: &Bindings,
    ) -> Vec<Option<ValueRef>> {
        match arg_tmpl {
            // Literal arguments always have exactly one possible value: itself.
            StatementTmplArg::Literal(v) => vec![Some(ValueRef::Literal(v.clone()))],

            // We do not attempt to infer a set of possible values free wildcards;
            // however, predicate handlers may attempt to deduce the value of a wildcard
            // at a later stage.
            StatementTmplArg::Wildcard(w) => match bindings.get(w) {
                Some(v) => vec![Some(ValueRef::Literal(v.clone()))], // bound
                None => vec![None],                                  // still free
            },

            // Anchored keys are more complex.
            // If the wildcard for the PodId is bound, then we can construct an anchored key.
            // If the wildcard for the PodId is free, then we can enumerate all anchored keys
            // for pods that have that key.
            StatementTmplArg::AnchoredKey(pod_wc, key) => match bindings.get(pod_wc) {
                // pod already bound
                Some(v) => match PodId::try_from(v.typed()) {
                    Ok(pid) => vec![Some(ValueRef::Key(AnchoredKey::new(pid, key.clone())))],
                    Err(_) => vec![], // binding can't be a PodId
                },

                // pod unbound – enumerate every pod that has that key
                None => self
                    .db
                    .get_pod_ids_with_key(key)
                    .into_iter()
                    .map(|pid| Some(ValueRef::Key(AnchoredKey::new(pid, key.clone()))))
                    .collect(),
            },

            _ => unreachable!("None args are not allowed in statement templates"),
        }
    }

    fn candidate_statement_args_from_bindings<'b>(
        &'b self,
        args: &'b [StatementTmplArg],
        bindings: &'b Bindings,
    ) -> impl Iterator<Item = Vec<Option<ValueRef>>> + 'b {
        args.iter()
            // Return a list of possible values for each argument
            .map(move |arg| self.column_choices(arg, bindings))
            .collect::<Vec<_>>()
            .into_iter()
            // We now have a list of lists, so we can enumerate all possible combinations.
            .multi_cartesian_product()
    }

    pub fn materialize_statements(
        &self,
        predicate: Predicate,
        args: Vec<StatementTmplArg>,
        bindings: &Bindings,
    ) -> Result<Relation, SolverError> {
        let key = MaterializeKey::from(&predicate, &args, bindings);
        if self.already_done(&key) {
            return Ok(Relation::new());
        }

        let rel: Relation = match &predicate {
            Predicate::Custom(cpr) => {
                let binding_vector: Vec<Option<Value>> = args
                    .iter()
                    .map(|arg| self.resolve_term(arg, bindings))
                    .collect();
                self.iter_custom_statements(cpr, &binding_vector)
                    .map(|(fact_values, source)| Fact {
                        source,
                        args: fact_values.into_iter().map(ValueRef::Literal).collect(),
                    })
                    .collect()
            }

            Predicate::Native(native_pred) => {
                let mut rel = Relation::new();

                // At this point, our binding vector can contain, in each slot:
                // - Nothing (None)
                // - A ValueRef resolving to an anchored key
                // - A Value
                //
                // From this, we can look up existing statements that match the pattern.
                // For example, Equal(?a["foo"], ?b["bar"]) will match a statement which
                // has those keys in those positions. If ?a and ?b are unbound, then we
                // will find all such statements. After doing so, we need to check that
                // the statements are true! If values for both anchored keys are known,
                // then we can do a value comparison. If not, then we can try other
                // strategies:
                // - If a matching statement exists in the DB, we can copy it
                // - For Equal, we can also attempt to construct a transitive equality
                //   path

                let candidate_args_iter =
                    self.candidate_statement_args_from_bindings(&args, bindings);

                // Ok, now we have our candidate args. We need to dispatch to the handler.
                let handler = PredicateHandler::for_native_predicate(*native_pred);

                for candidate_args in candidate_args_iter {
                    log::info!(
                        "Materializing {} for {:?}",
                        crate::pretty_print::PrettyValueRefVec(&candidate_args),
                        native_pred
                    );
                    // Hack
                    if *native_pred == NativePredicate::Equal {
                        if let Some(ValueRef::Key(ak)) = &candidate_args[0] {
                            if ak.pod_id == SELF {
                                rel.insert(Fact {
                                    source: FactSource::NewEntry,
                                    args: candidate_args
                                        .clone()
                                        .into_iter()
                                        .map(|arg| arg.unwrap())
                                        .collect(),
                                });
                                continue;
                            }
                        }
                    } else {
                        let new_rel = handler.materialize(&candidate_args, &self.db);
                        rel.extend(new_rel);
                    }
                }

                rel
            }
            Predicate::BatchSelf(_) => {
                unimplemented!("BatchSelf is not implemented")
            }
        };

        // Conditional DEBUG log for empty results to help debug materialization failures
        if let Predicate::Custom(_cpr) = &predicate {
            log::debug!(
                "{}",
                crate::pretty_print::PrettyMaterializationResult {
                    predicate: &crate::ir::PredicateIdentifier::Normal(predicate.clone()),
                    bindings,
                    result_count: rel.len(),
                }
            );
        }

        Ok(rel)
    }

    pub fn begin_iteration(&self) {
        self.materialised_keys.borrow_mut().clear();
    }

    fn already_done(&self, k: &MaterializeKey) -> bool {
        !self.materialised_keys.borrow_mut().insert(k.clone())
    }
}
