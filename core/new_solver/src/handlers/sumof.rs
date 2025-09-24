use pod2::middleware::{NativePredicate, StatementTmplArg};

use super::{
    ternary::TernaryArithmeticHandler,
    util::{arg_to_selector, handle_copy_results},
};
use crate::{edb::EdbView, op::OpHandler, prop::PropagatorResult, types::ConstraintStore};

/// Copy SumOf rows matching two-of-three syntactically, binding the third when wildcard or AK root wildcard.
pub struct CopySumOfHandler;

impl OpHandler for CopySumOfHandler {
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
            crate::edb::PredicateKey::Native(NativePredicate::SumOf),
            &[sel_a, sel_b, sel_c],
        );

        handle_copy_results(results, args, store)
    }
}

pub fn register_sumof_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::SumOf,
        Box::new(TernaryArithmeticHandler::new(
            |b, c| Some(b + c),
            |a, c| Some(a - c),
            |a, b| Some(a - b),
            "SumOf",
        )),
    );
    reg.register(NativePredicate::SumOf, Box::new(CopySumOfHandler));
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
    fn sumof_two_of_three_binds_wildcard() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b + c),
            |a, c| Some(a - c),
            |a, b| Some(a - b),
            "SumOf",
        );
        let args = args_from("REQUEST(SumOf(X, 3, 4))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { bindings, .. } => {
                assert_eq!(bindings, vec![(0, Value::from(7))]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn sumof_two_of_three_enumerates_for_ak_var() {
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("a"), Value::from(7))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        let edb = ImmutableEdbBuilder::new()
            .add_full_dict(dict)
            .add_statement_for_test(
                Statement::Contains(root.into(), "a".into(), 7.into()),
                PodRef(root),
            )
            .build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b + c),
            |a, c| Some(a - c),
            |a, b| Some(a - b),
            "SumOf",
        );
        let args = args_from("REQUEST(SumOf(R[\"a\"], 3, 4))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 0 && v.raw() == Value::from(root).raw())));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn sumof_all_ground_validates_with_premises_for_aks() {
        let params = Params::default();
        let d1 = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("x"), Value::from(3))].into(),
        )
        .unwrap();
        let d2 = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("y"), Value::from(4))].into(),
        )
        .unwrap();
        let r1 = d1.commitment();
        let r2 = d2.commitment();
        let edb = ImmutableEdbBuilder::new()
            .add_full_dict(d1)
            .add_full_dict(d2)
            .add_statement_for_test(
                Statement::Contains(r1.into(), "x".into(), 3.into()),
                PodRef(r1),
            )
            .add_statement_for_test(
                Statement::Contains(r2.into(), "y".into(), 4.into()),
                PodRef(r2),
            )
            .build();
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(r1));
        store.bindings.insert(1, Value::from(r2));
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b + c),
            |a, c| Some(a - c),
            |a, b| Some(a - b),
            "SumOf",
        );
        let args = args_from("REQUEST(SumOf(7, A[\"x\"], B[\"y\"]))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => match op_tag {
                crate::OpTag::Derived { premises } => assert_eq!(premises.len(), 2),
                other => panic!("unexpected tag: {other:?}"),
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn copy_sumof_matches_two_of_three_and_binds_third() {
        let src = PodRef(test_helpers::root("s"));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::SumOf(15.into(), 5.into(), 10.into()), src)
            .build();
        let mut store = ConstraintStore::default();
        let handler = CopySumOfHandler;
        // Match first two, bind third
        let args = args_from("REQUEST(SumOf(15, 5, Z))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 0 || (*i == 2 && *v == Value::from(10)))));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
