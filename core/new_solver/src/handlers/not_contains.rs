use pod2::middleware::{Hash, Key, NativePredicate, StatementTmplArg};

use super::util::{arg_to_selector, handle_copy_results};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
};

/// Copy NotContains(root, key) rows; supports binding root when key is known.
pub struct CopyNotContainsHandler;

impl OpHandler for CopyNotContainsHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }

        // We need to store owned values for selectors, since ArgSel holds references.
        let (mut l_val, mut l_root) = (None, None);
        let (mut r_val, mut r_root) = (None, None);

        let lhs = arg_to_selector(&args[0], store, &mut l_val, &mut l_root);
        let rhs = arg_to_selector(&args[1], store, &mut r_val, &mut r_root);

        let results = edb.query(
            crate::edb::PredicateKey::Native(pod2::middleware::NativePredicate::NotContains),
            &[lhs, rhs],
        );

        handle_copy_results(results, args, store)
    }
}

/// NotContainsFromEntries: if full dict known and key absent, entail when root bound.
pub struct NotContainsFromEntriesHandler;

impl OpHandler for NotContainsFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }
        let a_root = &args[0];
        let a_key = &args[1];

        // Handle literal container argument
        if let Some(container_val) = match a_root {
            StatementTmplArg::Literal(v) => Some(v.clone()),
            StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).cloned(),
            _ => None,
        } {
            match container_val.typed() {
                pod2::middleware::TypedValue::Dictionary(dict) => {
                    if let Some(key) = super::contains::key_from_arg(a_key, store) {
                        return match dict.get(&key) {
                            Ok(_) => PropagatorResult::Contradiction,
                            Err(_) => PropagatorResult::Entailed {
                                bindings: vec![],
                                op_tag: OpTag::FromLiterals,
                            },
                        };
                    }
                }
                pod2::middleware::TypedValue::Array(array) => {
                    if let Some(index) = match a_key {
                        StatementTmplArg::Literal(v) => match v.typed() {
                            pod2::middleware::TypedValue::Int(i) => Some(*i),
                            _ => None,
                        },
                        StatementTmplArg::Wildcard(w) => {
                            store.bindings.get(&w.index).and_then(|v| match v.typed() {
                                pod2::middleware::TypedValue::Int(i) => Some(*i),
                                _ => None,
                            })
                        }
                        _ => None,
                    } {
                        if array.get(index as usize).is_err() {
                            return PropagatorResult::Entailed {
                                bindings: vec![],
                                op_tag: OpTag::FromLiterals,
                            };
                        } else {
                            return PropagatorResult::Contradiction;
                        }
                    }
                }
                pod2::middleware::TypedValue::Set(set) => {
                    if let Some(value_to_check) = match a_key {
                        StatementTmplArg::Literal(v) => Some(v.clone()),
                        StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).cloned(),
                        _ => None,
                    } {
                        return match set.contains(&value_to_check) {
                            false => PropagatorResult::Entailed {
                                bindings: vec![],
                                op_tag: OpTag::FromLiterals,
                            },
                            true => PropagatorResult::Contradiction,
                        };
                    }
                }
                _ => {} // Fall through for EDB-based logic
            }
        }

        // Extract root hash if bound
        let root = match a_root {
            StatementTmplArg::Literal(v) => Some(Hash::from(v.raw())),
            StatementTmplArg::Wildcard(w) => {
                store.bindings.get(&w.index).map(|v| Hash::from(v.raw()))
            }
            _ => None,
        };
        // Extract key if literal or bound wildcard
        let key = match a_key {
            StatementTmplArg::Literal(v) => String::try_from(v.typed()).ok().map(Key::from),
            StatementTmplArg::Wildcard(w) => store
                .bindings
                .get(&w.index)
                .and_then(|v| String::try_from(v.typed()).ok().map(Key::from)),
            _ => None,
        };
        match (root, key) {
            (Some(r), Some(k)) => match edb.full_dict_absence(&r, &k) {
                Some(true) => PropagatorResult::Entailed {
                    bindings: vec![],
                    op_tag: OpTag::FromLiterals,
                },
                Some(false) => PropagatorResult::Contradiction,
                None => {
                    // Unknown absence; try copy path next
                    PropagatorResult::Contradiction
                }
            },
            (None, _) => {
                // Root unbound -> suspend on root wildcard
                let waits = crate::prop::wildcards_in_args(args)
                    .into_iter()
                    .filter(|i| !store.bindings.contains_key(i))
                    .collect::<Vec<_>>();
                if waits.is_empty() {
                    PropagatorResult::Contradiction
                } else {
                    PropagatorResult::Suspend { on: waits }
                }
            }
            _ => PropagatorResult::Contradiction,
        }
    }
}

pub fn register_not_contains_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::NotContains,
        Box::new(CopyNotContainsHandler),
    );
    reg.register(
        NativePredicate::NotContains,
        Box::new(NotContainsFromEntriesHandler),
    );
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{containers::Dictionary, Params, Statement, Value};

    use super::*;
    use crate::{
        edb::ImmutableEdbBuilder,
        test_helpers::{self, args_from},
        types::ConstraintStore,
    };

    #[test]
    fn not_contains_copy_binds_root_for_key() {
        let r = test_helpers::root("r");
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::NotContains(r.into(), "missing".into()),
                crate::types::PodRef(r),
            )
            .build();
        let mut store = ConstraintStore::default();
        let handler = CopyNotContainsHandler;
        let args = args_from("REQUEST(NotContains(?R, \"missing\"))");
        match handler.propagate(&args, &mut store, &edb) {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(_, v)| v.raw() == Value::from(r).raw())));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn not_contains_from_entries_entails_when_absent() {
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("x"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(r));
        let handler = NotContainsFromEntriesHandler;
        let args = args_from("REQUEST(NotContains(?R, \"missing\"))");
        match handler.propagate(&args, &mut store, &edb) {
            PropagatorResult::Entailed { .. } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn not_contains_from_entries_contradiction_when_present() {
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("y"), Value::from(2))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(r));
        let handler = NotContainsFromEntriesHandler;
        let args = args_from("REQUEST(NotContains(?R, \"y\"))");
        match handler.propagate(&args, &mut store, &edb) {
            PropagatorResult::Contradiction => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn not_contains_suspend_when_root_unbound() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = NotContainsFromEntriesHandler;
        let args = args_from("REQUEST(NotContains(?R, \"k\"))");
        match handler.propagate(&args, &mut store, &edb) {
            PropagatorResult::Suspend { on } => assert!(on.contains(&0)),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
