use pod2::middleware::{NativePredicate, StatementTmplArg};

use super::{
    binary::BinaryComparisonHandler,
    util::{arg_to_selector, handle_copy_results},
};
use crate::{edb::EdbView, op::OpHandler, prop::PropagatorResult, types::ConstraintStore};

/// Structural copy of NotEqual matching template shape; can bind wildcard value when AK root bound.
pub struct CopyNotEqualHandler;

impl OpHandler for CopyNotEqualHandler {
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
            crate::edb::PredicateKey::Native(NativePredicate::NotEqual),
            &[lhs, rhs],
        );

        handle_copy_results(results, args, store)
    }
}

pub fn register_not_equal_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::NotEqual,
        Box::new(BinaryComparisonHandler::new(|a, b| a != b, "NotEqual")),
    );
    reg.register(NativePredicate::NotEqual, Box::new(CopyNotEqualHandler));
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
    fn not_equal_from_entries_literals() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = BinaryComparisonHandler::new(|a, b| a != b, "NotEqual");
        let args = args_from("REQUEST(NotEqual(3, 5))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => {
                assert!(matches!(op_tag, OpTag::FromLiterals));
            }
            other => panic!("unexpected result: {other:?}"),
        }
        let args_false = args_from("REQUEST(NotEqual(5, 5))");
        let res2 = handler.propagate(&args_false, &mut store, &edb);
        assert!(matches!(res2, PropagatorResult::Contradiction));
    }

    #[test]
    fn not_equal_from_entries_ak_lit_generated() {
        // Lt(?R["k"], 10) with bound root and full dict k:7
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
        let handler = BinaryComparisonHandler::new(|a, b| a != b, "NotEqual");
        let args = args_from("REQUEST(NotEqual(?R[\"k\"], 10))");
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
    fn not_equal_from_entries_ak_ak_both_bound() {
        // NotEqual(?L["a"], ?R["b"]) with both bound and 3 != 5
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
        let handler = BinaryComparisonHandler::new(|a, b| a != b, "NotEqual");
        let args = args_from(r#"REQUEST(NotEqual(?L["a"], ?R["b"]))"#);
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
    fn not_equal_from_entries_suspend_unbound() {
        // NotEqual(?L["a"], 10) with unbound left root should suspend
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = BinaryComparisonHandler::new(|a, b| a != b, "NotEqual");
        let args = args_from("REQUEST(NotEqual(?L[\"a\"], 10))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Suspend { on } => assert!(on.contains(&0)),
            other => panic!("expected Suspend, got {other:?}"),
        }
    }

    #[test]
    fn copy_not_equal_binds_value_from_left_ak_when_root_bound() {
        // Given NotEqual(R["k"], 10) in EDB, CopyNotEqual should bind ?X when ?R bound
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
                Statement::NotEqual(
                    AnchoredKey::new(r, test_helpers::key("k")).into(),
                    10.into(),
                ),
                src.clone(),
            )
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(r)); // ?R
        let handler = CopyNotEqualHandler;
        let args = args_from(r#"REQUEST(Lt(?R["k"], ?X))"#);
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                let ch = &alternatives[0];
                assert_eq!(ch.bindings[0].0, 1); // ?X index
                assert_eq!(ch.bindings[0].1, Value::from(10));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn copy_not_equal_binds_root_from_right_ak_when_value_bound() {
        // Given NotEqual(10, R["k"]) in EDB, CopyNotEqual should bind ?R when ?X bound
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
                Statement::NotEqual(
                    10.into(),
                    AnchoredKey::new(r, test_helpers::key("k")).into(),
                ),
                src.clone(),
            )
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(10)); // ?X left
        let handler = CopyNotEqualHandler;
        let args = args_from(r#"REQUEST(Lt(?X, ?R["k"]))"#);
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
    fn copy_not_equal_binds_both_wildcards_from_vv_fact() {
        // NotEqual(?X, ?Y) should bind both from NotEqual(3, 5) fact
        let src = PodRef(test_helpers::root("s"));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::NotEqual(3.into(), 5.into()), src.clone())
            .build();

        let mut store = ConstraintStore::default();
        let handler = CopyNotEqualHandler;
        let args = args_from("REQUEST(NotEqual(?X, ?Y))");
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
    fn copy_not_equal_binds_one_wildcard_from_vv_partial() {
        // NotEqual(?X, 5) binds ?X from NotEqual(3,5); NotEqual(3, ?Y) binds ?Y from NotEqual(3,5)
        let src = PodRef(test_helpers::root("s"));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::NotEqual(3.into(), 5.into()), src.clone())
            .build();

        let mut store = ConstraintStore::default();
        let handler = CopyNotEqualHandler;
        let args1 = args_from("REQUEST(NotEqual(?X, 5))");
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

        let args2 = args_from("REQUEST(NotEqual(3, ?Y))");
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
