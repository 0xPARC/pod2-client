use pod2::middleware::{NativePredicate, StatementTmplArg};

use super::{
    binary::BinaryComparisonHandler,
    util::{arg_to_selector, handle_copy_results},
};
use crate::{edb::EdbView, op::OpHandler, prop::PropagatorResult, types::ConstraintStore};

/// Structural copy of LtEq matching template shape; can bind wildcard value when AK root bound.
pub struct CopyLtEqHandler;

impl OpHandler for CopyLtEqHandler {
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
            crate::edb::PredicateKey::Native(NativePredicate::LtEq),
            &[lhs, rhs],
        );

        handle_copy_results(results, args, store)
    }
}

pub fn register_lteq_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::LtEq,
        Box::new(BinaryComparisonHandler::new(|a, b| a <= b, "LtEq")),
    );
    reg.register(NativePredicate::LtEq, Box::new(CopyLtEqHandler));
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{containers::Dictionary, AnchoredKey, Params, Statement, Value};

    use super::*;
    use crate::{
        edb::ImmutableEdbBuilder,
        test_helpers::{self, args_from},
        types::ConstraintStore,
        OpTag,
    };

    #[test]
    fn lteq_from_entries_literals() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = BinaryComparisonHandler::new(|a, b| a <= b, "LtEq");
        let args = args_from("REQUEST(LtEq(5, 5))");
        let res = handler.propagate(&args, &mut store, &edb);
        assert!(matches!(
            res,
            PropagatorResult::Entailed {
                op_tag: OpTag::FromLiterals,
                ..
            }
        ));
        let args2 = args_from("REQUEST(LtEq(7, 5))");
        let res2 = handler.propagate(&args2, &mut store, &edb);
        assert!(matches!(res2, PropagatorResult::Contradiction));
    }

    #[test]
    fn lteq_from_entries_ak_lit_generated() {
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
        let handler = BinaryComparisonHandler::new(|a, b| a <= b, "LtEq");
        let args = args_from("REQUEST(LtEq(?R[\"k\"], 7))");
        let res = handler.propagate(&args, &mut store, &edb);
        assert!(matches!(
            res,
            PropagatorResult::Entailed {
                op_tag: OpTag::Derived { .. },
                ..
            }
        ));
    }

    #[test]
    fn lteq_from_entries_ak_ak_both_bound() {
        // LtEq(?L["a"], ?R["b"]) with both AK roots bound; 3 <= 5 should entail with two premises
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

        let handler = BinaryComparisonHandler::new(|a, b| a <= b, "LtEq");
        let args = args_from("REQUEST(LtEq(?L[\"a\"], ?R[\"b\"]))");
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
    fn lteq_from_entries_suspend_unbound() {
        // LtEq(?R["k"], 7) with unbound root should suspend
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = BinaryComparisonHandler::new(|a, b| a <= b, "LtEq");
        let args = args_from(r#"REQUEST(LtEq(?R["k"], 7))"#);
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Suspend { on } => assert!(on.contains(&0)),
            other => panic!("expected Suspend, got {other:?}"),
        }
    }

    #[test]
    fn lteq_from_entries_type_error() {
        // LtEq("foo", 5) should be a type error/contradiction; same for AK non-int
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = BinaryComparisonHandler::new(|a, b| a <= b, "LtEq");
        let args = args_from("REQUEST(LtEq(\"foo\", 5))");
        let res = handler.propagate(&args, &mut store, &edb);
        assert!(matches!(res, PropagatorResult::Contradiction));
    }

    #[test]
    fn copy_lteq_binds_both_from_vv_fact() {
        let src = crate::types::PodRef(test_helpers::root("s"));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::LtEq(3.into(), 5.into()), src)
            .build();

        let mut store = ConstraintStore::default();
        let handler = CopyLtEqHandler;
        let args = args_from("REQUEST(LtEq(?X, ?Y))");
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
    fn copy_lteq_binds_one_from_partial_vv() {
        let src = crate::types::PodRef(test_helpers::root("s"));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::LtEq(3.into(), 5.into()), src)
            .build();

        let mut store = ConstraintStore::default();
        let handler = CopyLtEqHandler;
        // Bind right from left literal
        let args1 = args_from("REQUEST(LtEq(3, ?Y))");
        let res1 = handler.propagate(&args1, &mut store, &edb);
        match res1 {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 0 || (*i == 1 && *v == Value::from(5)))));
            }
            other => panic!("unexpected result: {other:?}"),
        }

        // Bind left from right literal
        let args2 = args_from("REQUEST(LtEq(?X, 5))");
        let res2 = handler.propagate(&args2, &mut store, &edb);
        match res2 {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 0 && *v == Value::from(3))));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn copy_lteq_binds_root_from_left_ak_when_value_literal() {
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(10))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = crate::types::PodRef(r);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::LtEq(
                    AnchoredKey::new(r, test_helpers::key("k")).into(),
                    10.into(),
                ),
                src,
            )
            .build();

        let mut store = ConstraintStore::default();
        let handler = CopyLtEqHandler;
        let args = args_from("REQUEST(LtEq(?R[\"k\"], 10))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 0 && v.raw() == Value::from(r).raw())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn copy_lteq_binds_root_from_right_ak_when_left_literal() {
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(10))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = crate::types::PodRef(r);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::LtEq(5.into(), AnchoredKey::new(r, test_helpers::key("k")).into()),
                src,
            )
            .build();

        let mut store = ConstraintStore::default();
        let handler = CopyLtEqHandler;
        let args = args_from("REQUEST(LtEq(5, ?R[\"k\"]))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(_, v)| v.raw() == Value::from(r).raw())));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
