use pod2::middleware::{NativePredicate, StatementTmplArg};

use super::{
    binary::BinaryComparisonHandler,
    util::{arg_to_selector, handle_copy_results},
};
use crate::{edb::EdbView, op::OpHandler, prop::PropagatorResult, types::ConstraintStore};

/// Structural copy of Lt matching template shape; can bind wildcard value when AK root bound.
pub struct CopyLtHandler;

impl OpHandler for CopyLtHandler {
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
            crate::edb::PredicateKey::Native(NativePredicate::Lt),
            &[lhs, rhs],
        );

        handle_copy_results(results, args, store)
    }
}

pub fn register_lt_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::Lt,
        Box::new(BinaryComparisonHandler::new(|a, b| a < b, "Lt")),
    );
    reg.register(NativePredicate::Lt, Box::new(CopyLtHandler));
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{containers::Dictionary, AnchoredKey, Params, Statement, Value};

    use super::*;
    use crate::{
        edb::ImmutableEdbBuilder,
        test_helpers::{self, args_from},
        types::{ConstraintStore, PodRef},
        OpTag,
    };

    #[test]
    fn lt_from_entries_literals() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = BinaryComparisonHandler::new(|a, b| a < b, "Lt");
        let args = args_from("REQUEST(Lt(3, 5))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => {
                assert!(matches!(op_tag, OpTag::FromLiterals));
            }
            other => panic!("unexpected result: {other:?}"),
        }
        let args_false = args_from("REQUEST(Lt(5, 3))");
        let res2 = handler.propagate(&args_false, &mut store, &edb);
        assert!(matches!(res2, PropagatorResult::Contradiction));
    }

    #[test]
    fn lt_from_entries_ak_lit_generated() {
        // Lt(R["k"], 10) with bound root and full dict k:7
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(7))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(root));
        let handler = BinaryComparisonHandler::new(|a, b| a < b, "Lt");
        let args = args_from("REQUEST(Lt(R[\"k\"], 10))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => match op_tag {
                OpTag::Derived { premises } => {
                    assert_eq!(premises.len(), 1);
                    assert!(matches!(premises[0].1, OpTag::GeneratedContains { .. }));
                }
                other => panic!("unexpected tag: {other:?}"),
            },
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn lt_from_entries_ak_ak_both_bound() {
        // Lt(L["a"], R["b"]) with both bound and 3 < 5
        let params = Params::default();
        let dl = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("a"), Value::from(3))].into(),
        )
        .unwrap();
        let dr = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("b"), Value::from(5))].into(),
        )
        .unwrap();
        let rl = dl.commitment();
        let rr = dr.commitment();
        let edb = ImmutableEdbBuilder::new()
            .add_full_dict(dl)
            .add_full_dict(dr)
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(rl));
        store.bindings.insert(1, Value::from(rr));
        let handler = BinaryComparisonHandler::new(|a, b| a < b, "Lt");
        let args = args_from(r#"REQUEST(Lt(L["a"], R["b"]))"#);
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => match op_tag {
                OpTag::Derived { premises } => assert_eq!(premises.len(), 2),
                other => panic!("unexpected tag: {other:?}"),
            },
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn lt_from_entries_suspend_unbound() {
        // Lt(L["a"], 10) with unbound left root should suspend
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = BinaryComparisonHandler::new(|a, b| a < b, "Lt");
        let args = args_from("REQUEST(Lt(L[\"a\"], 10))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Suspend { on } => assert!(on.contains(&0)),
            other => panic!("expected Suspend, got {other:?}"),
        }
    }

    #[test]
    fn copy_lt_binds_value_from_left_ak_when_root_bound() {
        // Given Lt(R["k"], 10) in EDB, CopyLt should bind X when R bound
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(7))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = PodRef(r);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::Lt(
                    AnchoredKey::new(r, test_helpers::key("k")).into(),
                    10.into(),
                ),
                src.clone(),
            )
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(r)); // R
        let handler = CopyLtHandler;
        let args = args_from(r#"REQUEST(Lt(R["k"], X))"#);
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                let ch = &alternatives[0];
                assert_eq!(ch.bindings[0].0, 1); // X index
                assert_eq!(ch.bindings[0].1, Value::from(10));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn copy_lt_binds_root_from_right_ak_when_value_bound() {
        // Given Lt(10, R["k"]) in EDB, CopyLt should bind R when X bound
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(20))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = PodRef(r);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::Lt(
                    10.into(),
                    AnchoredKey::new(r, test_helpers::key("k")).into(),
                ),
                src.clone(),
            )
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(10)); // X left
        let handler = CopyLtHandler;
        let args = args_from(r#"REQUEST(Lt(X, R["k"]))"#);
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 1 && v.raw() == Value::from(r).raw())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn copy_lt_binds_both_wildcards_from_vv_fact() {
        // Lt(X, Y) should bind both from Lt(3, 5) fact
        let src = PodRef(test_helpers::root("s"));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::Lt(3.into(), 5.into()), src.clone())
            .build();

        let mut store = ConstraintStore::default();
        let handler = CopyLtHandler;
        let args = args_from("REQUEST(Lt(X, Y))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 0 && *v == Value::from(3))));
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 1 && *v == Value::from(5))));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn copy_lt_binds_one_wildcard_from_vv_partial() {
        // Lt(X, 5) binds X from Lt(3,5); Lt(3, Y) binds Y from Lt(3,5)
        let src = PodRef(test_helpers::root("s"));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::Lt(3.into(), 5.into()), src.clone())
            .build();

        let mut store = ConstraintStore::default();
        let handler = CopyLtHandler;
        let args1 = args_from("REQUEST(Lt(X, 5))");
        let res1 = handler.propagate(&args1, &mut store, &edb);
        match res1 {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 0 && *v == Value::from(3))));
            }
            other => panic!("unexpected result: {other:?}"),
        }

        let args2 = args_from("REQUEST(Lt(3, Y))");
        let res2 = handler.propagate(&args2, &mut store, &edb);
        match res2 {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives
                    .iter()
                    .any(|ch| ch.bindings.iter().any(|(_, v)| *v == Value::from(5))));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
