use pod2::middleware::{Hash, Key, NativePredicate, StatementTmplArg, Value};

use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
};

/// Utility: extract a bound root hash from a template arg (literal or wildcard).
fn root_from_arg(arg: &StatementTmplArg, store: &ConstraintStore) -> Option<Hash> {
    match arg {
        StatementTmplArg::Literal(v) => Some(Hash::from(v.raw())),
        StatementTmplArg::Wildcard(w) => store.bindings.get(&w.index).map(|v| Hash::from(v.raw())),
        _ => None,
    }
}

/// Utility: extract a Key from a template arg (literal string or wildcard bound to string).
fn key_from_arg(arg: &StatementTmplArg, store: &ConstraintStore) -> Option<Key> {
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
        let (a_root, a_key, a_val) = (&args[0], &args[1], &args[2]);
        // Need root and key to proceed
        let root = match root_from_arg(a_root, store) {
            Some(r) => r,
            None => {
                // If either wildcard is unbound, suspend on them
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
            // Bind the value wildcard from copied facts
            StatementTmplArg::Wildcard(wv) => {
                let mut alts = Vec::new();
                for (v, src) in edb.contains_copied_values(&root, &key) {
                    alts.push(crate::prop::Choice {
                        bindings: vec![(wv.index, v)],
                        op_tag: OpTag::CopyStatement { source: src },
                    });
                }
                if alts.is_empty() {
                    // No copied fact to bind value → Contradiction (lets other handlers try)
                    PropagatorResult::Contradiction
                } else {
                    PropagatorResult::Choices { alternatives: alts }
                }
            }
            // Literal or bound wildcard value: check for copied provenance
            StatementTmplArg::Literal(v) => match edb.contains_source(&root, &key, v) {
                Some(crate::edb::ContainsSource::Copied { pod }) => PropagatorResult::Entailed {
                    bindings: vec![],
                    op_tag: OpTag::CopyStatement { source: pod },
                },
                _ => PropagatorResult::Contradiction,
            },
            StatementTmplArg::AnchoredKey(_, _) | StatementTmplArg::None => {
                // Not a valid value for Contains; fail this branch
                PropagatorResult::Contradiction
            }
        }
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
    use pod2::middleware::{containers::Dictionary, Params, StatementTmplArg, Value};

    use super::*;
    use crate::{
        edb::MockEdbView,
        test_helpers,
        types::{ConstraintStore, PodRef},
    };

    fn args_from(query: &str) -> Vec<StatementTmplArg> {
        let tmpl = test_helpers::parse_first_tmpl(query);
        tmpl.args().to_vec()
    }

    #[test]
    fn copy_contains_binds_value_when_root_key_known() {
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(7))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        let pod = PodRef(root);
        edb.add_copied_contains(root, test_helpers::key("k"), Value::from(7), pod.clone());

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
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(9))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        edb.add_full_dict(dict);

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
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        // Both copied and full
        edb.add_copied_contains(root, test_helpers::key("k"), Value::from(1), PodRef(root));
        edb.add_full_dict(dict);

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
