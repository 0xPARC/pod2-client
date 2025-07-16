use std::collections::{HashMap, HashSet};

use pod2::middleware::{
    AnchoredKey, PodId, Predicate, StatementTmpl, StatementTmplArg, Value, ValueRef, Wildcard,
};

use crate::{
    engine::semi_naive::{Bindings, Fact, FactStore, SemiNaiveEngine},
    error::SolverError,
    ir::{Atom, PredicateIdentifier, Rule},
    metrics::NoOpMetrics,
    semantics::materializer::Materializer,
};

type MissingAtom = StatementTmpl;

pub struct MissingFactFinder<'a> {
    all_facts: &'a FactStore,
    materializer: &'a Materializer,
}

impl<'a> MissingFactFinder<'a> {
    pub fn new(all_facts: &'a FactStore, materializer: &'a Materializer) -> Self {
        Self {
            all_facts,
            materializer,
        }
    }

    /// Returns every atom that caused a join failure in every
    /// guarded rule reachable from the request.
    pub fn collect(&self, rules: &[Rule]) -> Vec<MissingAtom> {
        let mut seen: HashSet<MissingAtom> = HashSet::new();
        let mut ordered = Vec::new();

        let mut interim = Vec::new();
        for rule in rules {
            self.replay_rule(rule, &HashMap::new(), &mut interim);
        }

        for lit in interim.into_iter() {
            if seen.insert(lit.clone()) {
                ordered.push(lit);
            }
        }

        ordered
    }

    // ----------------------------------------------------------------
    // replay_rule ≈ stripped-down version of `perform_join`
    // ----------------------------------------------------------------
    fn replay_rule(&self, rule: &Rule, seed: &Bindings, out: &mut Vec<MissingAtom>) {
        // Determine external (public) wildcards from rule head
        let externals: HashSet<Wildcard> = rule
            .head
            .terms
            .iter()
            .filter_map(|t| match t {
                StatementTmplArg::Wildcard(wc) => Some(wc.clone()),
                StatementTmplArg::AnchoredKey(pod_wc, _) => Some(pod_wc.clone()),
                _ => None,
            })
            .collect();

        self.replay_rule_inner(rule, seed, &externals, out);
    }

    fn replay_rule_inner(
        &self,
        rule: &Rule,
        seed: &Bindings,
        externals: &HashSet<Wildcard>,
        out: &mut Vec<MissingAtom>,
    ) {
        let mut current: Vec<Bindings> = vec![seed.clone()];
        let mut invalid: HashSet<Wildcard> = HashSet::new();

        for atom in &rule.body {
            // If atom uses an invalidated wildcard, treat as failed immediately
            if self.atom_mentions_invalid(atom, &invalid) {
                if matches!(
                    atom.predicate,
                    PredicateIdentifier::Normal(Predicate::Native(_))
                ) && !self.is_impossible_native(atom, &current[0])
                {
                    out.push(self.partial_instantiate(atom, &current[0], externals));
                }
                invalid.extend(self.wildcards_in_atom(atom));
                continue;
            }

            // Otherwise attempt join
            let mut next = Vec::new();
            for b in &current {
                let rel = self.fetch_relation(atom, b);
                for fact in &rel {
                    if let Some(nb) = self.unify(b, atom, &fact.args).ok().flatten() {
                        next.push(nb);
                    }
                }
            }

            if next.is_empty() {
                // Atom truly fails under current bindings
                if matches!(
                    atom.predicate,
                    PredicateIdentifier::Normal(Predicate::Native(_))
                ) && !self.is_impossible_native(atom, &current[0])
                {
                    out.push(self.partial_instantiate(atom, &current[0], externals));
                }
                invalid.extend(self.wildcards_in_atom(atom));
                // We continue scanning tail so that later atoms that depend on these
                // wildcards are captured as failed.
                continue;
            }

            current = next;
        }
    }

    fn atom_mentions_invalid(&self, atom: &Atom, invalid: &HashSet<Wildcard>) -> bool {
        atom.terms.iter().any(|t| match t {
            StatementTmplArg::Wildcard(wc) => invalid.contains(wc),
            StatementTmplArg::AnchoredKey(wc, _) => invalid.contains(wc),
            _ => false,
        })
    }

    fn wildcards_in_atom(&self, atom: &Atom) -> HashSet<Wildcard> {
        let mut set = HashSet::new();
        for t in &atom.terms {
            match t {
                StatementTmplArg::Wildcard(wc) => {
                    set.insert(wc.clone());
                }
                StatementTmplArg::AnchoredKey(wc, _) => {
                    set.insert(wc.clone());
                }
                _ => {}
            }
        }
        set
    }

    // ------------------------------------------------------------
    // helpers (all borrowed from existing engine code)
    // ------------------------------------------------------------
    fn fetch_relation(&self, atom: &Atom, b: &Bindings) -> Vec<Fact> {
        match &atom.predicate {
            PredicateIdentifier::Normal(pred) => {
                // EDB + IDB via materializer
                self.materializer
                    .materialize_statements(pred.clone(), atom.terms.clone(), b)
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
            }
            PredicateIdentifier::Magic { .. } => {
                // Magic predicates are IDB-only – look them up directly in the fact store.
                self.all_facts
                    .get(&atom.predicate)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
            }
        }
    }

    fn unify(
        &self,
        b: &Bindings,
        atom: &Atom,
        fact: &[ValueRef],
    ) -> Result<Option<Bindings>, SolverError> {
        SemiNaiveEngine::<NoOpMetrics>::default().unify(b, atom, fact)
    }

    fn partial_instantiate(
        &self,
        atom: &Atom,
        b: &Bindings,
        externals: &HashSet<Wildcard>,
    ) -> MissingAtom {
        let args = atom
            .terms
            .iter()
            .map(|t| match t {
                StatementTmplArg::Wildcard(wc) if externals.contains(wc) => b
                    .get(wc)
                    .map(|v| StatementTmplArg::Literal(v.clone()))
                    .unwrap_or(t.clone()),
                StatementTmplArg::AnchoredKey(pod_wc, key) if externals.contains(pod_wc) => {
                    // Keep anchored key but ensure pod wildcard remains same placeholder
                    StatementTmplArg::AnchoredKey(pod_wc.clone(), key.clone())
                }
                _ => t.clone(),
            })
            .collect();
        MissingAtom {
            pred: match &atom.predicate {
                PredicateIdentifier::Normal(p) => p.clone(),
                _ => unreachable!(),
            },
            args,
        }
    }

    fn is_impossible_native(&self, atom: &Atom, b: &Bindings) -> bool {
        if let PredicateIdentifier::Normal(Predicate::Native(native_pred)) = &atom.predicate {
            // Try to resolve all terms to concrete Values
            let maybe_values: Option<Vec<Value>> = atom
                .terms
                .iter()
                .map(|t| match t {
                    StatementTmplArg::Literal(v) => Some(v.clone()),
                    StatementTmplArg::Wildcard(wc) => b.get(wc).cloned(),
                    StatementTmplArg::AnchoredKey(pod_wc, key) => b.get(pod_wc).and_then(|val| {
                        if let Ok(pid) = PodId::try_from(val.typed()) {
                            let ak = AnchoredKey::new(pid, key.clone());
                            self.materializer.value_ref_to_value(&ValueRef::Key(ak))
                        } else {
                            None
                        }
                    }),
                    _ => None,
                })
                .collect();

            if let Some(vals) = maybe_values {
                use pod2::middleware::NativePredicate as NP;
                match native_pred {
                    NP::Equal => return vals[0] != vals[1],
                    NP::NotEqual => return vals[0] == vals[1],
                    _ => {}
                }
            }
        }
        false
    }
}
