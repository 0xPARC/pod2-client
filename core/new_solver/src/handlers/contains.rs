use pod2::middleware::{Hash, Key, NativePredicate, StatementTmplArg, TypedValue, Value};

use super::util::{arg_to_selector, handle_copy_results};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
};

/// Utility: extract a bound root hash from a template arg (literal or wildcard).
pub fn root_from_arg(arg: &StatementTmplArg, store: &ConstraintStore) -> Option<Hash> {
    match arg {
        StatementTmplArg::Literal(v) => Some(Hash::from(v.raw())),
        StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).map(|v| Hash::from(v.raw())),
        _ => None,
    }
}

/// Utility: extract a Key from a template arg (literal string or wildcard bound to string).
pub fn key_from_arg(arg: &StatementTmplArg, store: &ConstraintStore) -> Option<Key> {
    match arg {
        StatementTmplArg::Literal(v) => {
            if let Ok(s) = String::try_from(v.typed()) {
                Some(Key::from(s))
            } else {
                None
            }
        }
        StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).and_then(|v| {
            if let Ok(s) = String::try_from(v.typed()) {
                Some(Key::from(s))
            } else {
                None
            }
        }),
        _ => None,
    }
}

/// Copy existing Contains(root, key, value) statements from EDB.
/// Supports binding the value (third argument) when root and key are known.
pub struct CopyContainsHandler;

impl OpHandler for CopyContainsHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 3 {
            return PropagatorResult::Contradiction;
        }

        // We need to store owned values for selectors, since ArgSel holds references.
        let (mut a_val, mut a_root) = (None, None);
        let (mut b_val, mut b_root) = (None, None);
        let (mut c_val, mut c_root) = (None, None);

        let sel_a = arg_to_selector(&args[0], store, &mut a_val, &mut a_root);
        let sel_b = arg_to_selector(&args[1], store, &mut b_val, &mut b_root);
        let sel_c = arg_to_selector(&args[2], store, &mut c_val, &mut c_root);

        let results = edb.query(
            crate::edb::PredicateKey::Native(NativePredicate::Contains),
            &[sel_a, sel_b, sel_c],
        );

        handle_copy_results(results, args, store)
    }
}

/// ContainsFromEntries: when the full dictionary is known, it can justify Contains and bind value.
pub struct ContainsFromEntriesHandler;

impl OpHandler for ContainsFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 3 {
            return PropagatorResult::Contradiction;
        }
        let (a_root, a_key, a_val) = (&args[0], &args[1], &args[2]);

        // Handle literal container argument
        if let Some(container_val) = match a_root {
            StatementTmplArg::Literal(v) => Some(v.clone()),
            StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).cloned(),
            _ => None,
        } {
            match container_val.typed() {
                pod2::middleware::TypedValue::Dictionary(dict) => {
                    if let Some(key) = key_from_arg(a_key, store) {
                        return match dict.get(&key) {
                            Ok(dict_value) => match a_val {
                                StatementTmplArg::Literal(v) => {
                                    if dict_value == v {
                                        PropagatorResult::Entailed {
                                            bindings: vec![],
                                            op_tag: OpTag::FromLiterals,
                                        }
                                    } else {
                                        PropagatorResult::Contradiction
                                    }
                                }
                                StatementTmplArg::Wildcard(wv) => {
                                    if let Some(existing) = store.bindings.get(&wv.index) {
                                        if existing == dict_value {
                                            PropagatorResult::Entailed {
                                                bindings: vec![],
                                                op_tag: OpTag::FromLiterals,
                                            }
                                        } else {
                                            PropagatorResult::Contradiction
                                        }
                                    } else {
                                        PropagatorResult::Entailed {
                                            bindings: vec![(wv.index, dict_value.clone())],
                                            op_tag: OpTag::FromLiterals,
                                        }
                                    }
                                }
                                _ => PropagatorResult::Contradiction,
                            },
                            Err(_) => PropagatorResult::Contradiction, // Key not found
                        };
                    }
                }
                pod2::middleware::TypedValue::Array(array) => {
                    // Index must be a bound integer
                    if let Some(index) = match a_key {
                        StatementTmplArg::Literal(v) => match v.typed() {
                            TypedValue::Int(i) => Some(i),
                            _ => None,
                        },
                        StatementTmplArg::Wildcard(w) => {
                            store.bindings.get(&w.index).and_then(|v| match v.typed() {
                                TypedValue::Int(i) => Some(i),
                                _ => None,
                            })
                        }
                        _ => None,
                    } {
                        if *index < 0 {
                            return PropagatorResult::Contradiction;
                        }
                        return match array.get(*index as usize) {
                            Ok(array_value) => match a_val {
                                StatementTmplArg::Literal(v) => {
                                    if array_value == v {
                                        PropagatorResult::Entailed {
                                            bindings: vec![],
                                            op_tag: OpTag::FromLiterals,
                                        }
                                    } else {
                                        PropagatorResult::Contradiction
                                    }
                                }
                                StatementTmplArg::Wildcard(wv) => {
                                    if let Some(existing) = store.bindings.get(&wv.index) {
                                        if existing == array_value {
                                            PropagatorResult::Entailed {
                                                bindings: vec![],
                                                op_tag: OpTag::FromLiterals,
                                            }
                                        } else {
                                            PropagatorResult::Contradiction
                                        }
                                    } else {
                                        PropagatorResult::Entailed {
                                            bindings: vec![(wv.index, array_value.clone())],
                                            op_tag: OpTag::FromLiterals,
                                        }
                                    }
                                }
                                _ => PropagatorResult::Contradiction,
                            },
                            Err(_) => PropagatorResult::Contradiction, // Index out of bounds or other error
                        };
                    }
                }
                pod2::middleware::TypedValue::Set(set) => {
                    // For Sets, key and value arguments must unify to the same value.
                    let (value_opt, bindings_opt) = match (a_key, a_val) {
                        (StatementTmplArg::Literal(k), StatementTmplArg::Literal(v)) => {
                            if k != v {
                                return PropagatorResult::Contradiction;
                            }
                            (Some(k.clone()), Some(vec![]))
                        }
                        (StatementTmplArg::Literal(k), StatementTmplArg::Wildcard(wv)) => {
                            if let Some(bound_v) = store.bindings.get(&wv.index) {
                                if k != bound_v {
                                    return PropagatorResult::Contradiction;
                                }
                                (Some(k.clone()), Some(vec![]))
                            } else {
                                (Some(k.clone()), Some(vec![(wv.index, k.clone())]))
                            }
                        }
                        (StatementTmplArg::Wildcard(wk), StatementTmplArg::Literal(v)) => {
                            if let Some(bound_k) = store.bindings.get(&wk.index) {
                                if v != bound_k {
                                    return PropagatorResult::Contradiction;
                                }
                                (Some(v.clone()), Some(vec![]))
                            } else {
                                (Some(v.clone()), Some(vec![(wk.index, v.clone())]))
                            }
                        }
                        (StatementTmplArg::Wildcard(wk), StatementTmplArg::Wildcard(wv)) => {
                            let k_bound = store.bindings.get(&wk.index);
                            let v_bound = store.bindings.get(&wv.index);
                            match (k_bound, v_bound) {
                                (Some(k), Some(v)) => {
                                    if k != v {
                                        return PropagatorResult::Contradiction;
                                    }
                                    (Some(k.clone()), Some(vec![]))
                                }
                                (Some(k), None) => {
                                    (Some(k.clone()), Some(vec![(wv.index, k.clone())]))
                                }
                                (None, Some(v)) => {
                                    (Some(v.clone()), Some(vec![(wk.index, v.clone())]))
                                }
                                (None, None) => (None, None), // Cannot determine value if both unbound
                            }
                        }
                        _ => (None, None),
                    };

                    if let (Some(value_to_check), Some(bindings)) = (value_opt, bindings_opt) {
                        return match set.contains(&value_to_check) {
                            true => PropagatorResult::Entailed {
                                bindings,
                                op_tag: OpTag::FromLiterals,
                            },
                            false => PropagatorResult::Contradiction,
                        };
                    }
                }
                _ => {} // Fall through for other types to handle via EDB
            }
        }

        // Enumeration: if root is an unbound wildcard and key/value are known, enumerate candidate roots.
        if let StatementTmplArg::Wildcard(wr) = a_root {
            if !store.bindings.contains_key(&wr.index) {
                let key_opt = key_from_arg(a_key, store);
                let val_opt: Option<Value> = match a_val {
                    StatementTmplArg::Literal(v) => Some(v.clone()),
                    StatementTmplArg::Wildcard(wv) => store.bindings.get(&wv.index).cloned(),
                    _ => None,
                };
                if let (Some(key), Some(val)) = (key_opt, val_opt) {
                    let mut alts = Vec::new();
                    for (root, src) in edb.enumerate_contains_sources(&key, &val) {
                        let op_tag = match src {
                            crate::edb::ContainsSource::GeneratedFromFullDict { .. } => {
                                OpTag::GeneratedContains {
                                    root,
                                    key: key.clone(),
                                    value: val.clone(),
                                }
                            }
                            crate::edb::ContainsSource::Copied { pod } => {
                                OpTag::CopyStatement { source: pod }
                            }
                        };
                        alts.push(crate::prop::Choice {
                            bindings: vec![(wr.index, Value::from(root))],
                            op_tag,
                        });
                    }
                    tracing::trace!(?key, ?val, candidates = alts.len(), "Contains enum roots");
                    return if alts.is_empty() {
                        PropagatorResult::Contradiction
                    } else {
                        PropagatorResult::Choices { alternatives: alts }
                    };
                }
            }
        }
        // Need root and key to proceed
        let root = match root_from_arg(a_root, store) {
            Some(r) => r,
            None => {
                let waits = crate::prop::wildcards_in_args(args)
                    .into_iter()
                    .filter(|i| !store.bindings.contains_key(i))
                    .collect::<Vec<_>>();
                return if waits.is_empty() {
                    PropagatorResult::Contradiction
                } else {
                    PropagatorResult::Suspend { on: waits }
                };
            }
        };
        let key = match key_from_arg(a_key, store) {
            Some(k) => k,
            None => return PropagatorResult::Contradiction,
        };

        match a_val {
            // Bind the value from the full dictionary only
            StatementTmplArg::Wildcard(wv) => {
                if let Some(v) = edb.contains_full_value(&root, &key) {
                    return PropagatorResult::Entailed {
                        bindings: vec![(wv.index, v.clone())],
                        op_tag: OpTag::GeneratedContains {
                            root,
                            key: key.clone(),
                            value: v,
                        },
                    };
                }
                PropagatorResult::Contradiction
            }
            StatementTmplArg::Literal(v) => match edb.contains_source(&root, &key, v) {
                Some(crate::edb::ContainsSource::GeneratedFromFullDict { .. }) => {
                    PropagatorResult::Entailed {
                        bindings: vec![],
                        op_tag: OpTag::GeneratedContains {
                            root,
                            key: key.clone(),
                            value: v.clone(),
                        },
                    }
                }
                _ => PropagatorResult::Contradiction,
            },
            _ => PropagatorResult::Contradiction,
        }
    }
}

pub fn register_contains_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(NativePredicate::Contains, Box::new(CopyContainsHandler));
    reg.register(
        NativePredicate::Contains,
        Box::new(ContainsFromEntriesHandler),
    );
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{containers::Dictionary, Params, Statement, Value};

    use super::*;
    use crate::{
        edb::ImmutableEdbBuilder,
        test_helpers::{self, args_from},
        types::{ConstraintStore, PodRef},
    };

    #[test]
    fn copy_contains_binds_value_when_root_key_known() {
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(7))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        let pod = PodRef(root);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::Contains(root.into(), "k".into(), 7.into()),
                pod.clone(),
            )
            .build();

        let mut store = ConstraintStore::default();
        // Bind root and key via wildcards or literals; here we bind root as wildcard
        store.bindings.insert(0, Value::from(root));
        let handler = CopyContainsHandler;
        let args = args_from("REQUEST(Contains(?R, \"k\", ?V))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                let ch = &alternatives[0];
                assert_eq!(ch.bindings[0].0, 1); // ?V index
                assert_eq!(ch.bindings[0].1, Value::from(7));
                assert!(matches!(ch.op_tag, OpTag::CopyStatement { .. }));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn contains_from_entries_binds_value_from_full_dict() {
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(9))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(root));
        let handler = ContainsFromEntriesHandler;
        let args = args_from("REQUEST(Contains(?R, \"k\", ?V))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { bindings, op_tag } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].0, 1); // ?V index
                assert_eq!(bindings[0].1, Value::from(9));
                assert!(matches!(op_tag, OpTag::GeneratedContains { .. }));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn contains_handlers_prefer_generated_when_both_exist() {
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        // Both copied and full
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::Contains(root.into(), "k".into(), 1.into()),
                PodRef(root),
            )
            .add_full_dict(dict)
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(root));

        // Both handlers applicable; ContainsFromEntries yields Entailed, CopyContains yields Choices.
        // Engine will prefer GeneratedContains when deduping; here we just check individual handler outputs are reasonable.
        let copy = CopyContainsHandler;
        let gen = ContainsFromEntriesHandler;
        let args = args_from("REQUEST(Contains(?R, \"k\", ?V))");
        let r1 = copy.propagate(&args, &mut store.clone(), &edb);
        let r2 = gen.propagate(&args, &mut store.clone(), &edb);
        assert!(matches!(r1, PropagatorResult::Choices { .. }));
        match r2 {
            PropagatorResult::Entailed { op_tag, .. } => {
                assert!(matches!(op_tag, OpTag::GeneratedContains { .. }));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
