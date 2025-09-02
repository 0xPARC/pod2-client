use pod2::middleware::{Hash, Key, NativePredicate, StatementTmplArg, Value};

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
        let a_root = &args[0];
        let a_key = &args[1];
        match (a_root, a_key) {
            (StatementTmplArg::Wildcard(wr), StatementTmplArg::Literal(vk)) => {
                if let Ok(sk) = String::try_from(vk.typed()) {
                    let key = Key::from(sk);
                    let mut alts = Vec::new();
                    for (root, src) in edb.not_contains_roots_for_key(&key) {
                        alts.push(crate::prop::Choice {
                            bindings: vec![(wr.index, Value::from(root))],
                            op_tag: OpTag::CopyStatement { source: src },
                        });
                    }
                    if alts.is_empty() {
                        PropagatorResult::Contradiction
                    } else {
                        PropagatorResult::Choices { alternatives: alts }
                    }
                } else {
                    PropagatorResult::Contradiction
                }
            }
            (StatementTmplArg::Literal(vr), StatementTmplArg::Literal(vk)) => {
                let root = Hash::from(vr.raw());
                if let Ok(sk) = String::try_from(vk.typed()) {
                    let key = Key::from(sk);
                    if let Some(src) = edb.not_contains_copy_root_key(&root, &key) {
                        return PropagatorResult::Entailed {
                            bindings: vec![],
                            op_tag: OpTag::CopyStatement { source: src },
                        };
                    }
                }
                PropagatorResult::Contradiction
            }
            (StatementTmplArg::Wildcard(_wr), StatementTmplArg::Wildcard(_wk)) => {
                // Avoid enumerating keys; cannot bind safely
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
                // Root unbound → suspend on root wildcard
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
    use pod2::middleware::{containers::Dictionary, Params};

    use super::*;
    use crate::{
        edb::MockEdbView,
        test_helpers::{self, args_from},
        types::ConstraintStore,
    };

    #[test]
    fn not_contains_copy_binds_root_for_key() {
        let mut edb = MockEdbView::default();
        let r = test_helpers::root("r");
        edb.add_not_contains_row(r, test_helpers::key("missing"), crate::types::PodRef(r));
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
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("x"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        edb.add_full_dict(dict);
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
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("y"), Value::from(2))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        edb.add_full_dict(dict);
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
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = NotContainsFromEntriesHandler;
        let args = args_from("REQUEST(NotContains(?R, \"k\"))");
        match handler.propagate(&args, &mut store, &edb) {
            PropagatorResult::Suspend { on } => assert!(on.contains(&0)),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
