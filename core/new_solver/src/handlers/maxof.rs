use pod2::middleware::{NativePredicate, StatementTmplArg};

use super::{
    ternary::TernaryArithmeticHandler,
    util::{arg_to_selector, handle_copy_results},
};
use crate::{edb::EdbView, op::OpHandler, prop::PropagatorResult, types::ConstraintStore};

pub fn register_maxof_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::MaxOf,
        Box::new(TernaryArithmeticHandler::new(
            |b, c| Some(b.max(c)),
            |a, c| if a >= c { Some(a) } else { None },
            |a, b| if a >= b { Some(a) } else { None },
            "MaxOf",
        )),
    );
    reg.register(NativePredicate::MaxOf, Box::new(CopyMaxOfHandler));
}

/// CopyMaxOf: copy rows from EDB: matches any two-of-three and binds the third.
pub struct CopyMaxOfHandler;

impl OpHandler for CopyMaxOfHandler {
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
            crate::edb::PredicateKey::Native(NativePredicate::MaxOf),
            &[sel_a, sel_b, sel_c],
        );

        handle_copy_results(results, args, store)
    }
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{Statement, Value};

    use super::*;
    use crate::{
        edb::ImmutableEdbBuilder,
        test_helpers::{self, args_from},
        types::ConstraintStore,
    };

    #[test]
    fn maxof_two_of_three_binds_wildcard() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b.max(c)),
            |a, c| if a >= c { Some(a) } else { None },
            |a, b| if a >= b { Some(a) } else { None },
            "MaxOf",
        );
        let args = args_from("REQUEST(MaxOf(X, 3, 7))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { bindings, .. } => {
                assert_eq!(bindings, vec![(0, Value::from(7))]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn maxof_all_ground_validates() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b.max(c)),
            |a, c| if a >= c { Some(a) } else { None },
            |a, b| if a >= b { Some(a) } else { None },
            "MaxOf",
        );
        let args = args_from("REQUEST(MaxOf(7, 3, 7))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { .. } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn maxof_all_ground_mismatch_contradicts() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b.max(c)),
            |a, c| if a >= c { Some(a) } else { None },
            |a, b| if a >= b { Some(a) } else { None },
            "MaxOf",
        );
        let args = args_from("REQUEST(MaxOf(5, 3, 7))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Contradiction => {}
            other => panic!("expected contradiction, got: {other:?}"),
        }
    }

    #[test]
    fn maxof_solves_b_when_a_ge_c() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b.max(c)),
            |a, c| if a >= c { Some(a) } else { None },
            |a, b| if a >= b { Some(a) } else { None },
            "MaxOf",
        );
        // MaxOf(7, B, 3): max(B, 3) = 7, so B = 7
        let args = args_from("REQUEST(MaxOf(7, B, 3))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { bindings, .. } => {
                assert_eq!(bindings, vec![(0, Value::from(7))]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn maxof_solves_c_when_a_ge_b() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b.max(c)),
            |a, c| if a >= c { Some(a) } else { None },
            |a, b| if a >= b { Some(a) } else { None },
            "MaxOf",
        );
        // MaxOf(7, 3, C): max(3, C) = 7, so C = 7
        let args = args_from("REQUEST(MaxOf(7, 3, C))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { bindings, .. } => {
                assert_eq!(bindings, vec![(0, Value::from(7))]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn maxof_contradicts_when_a_lt_c_solving_for_b() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b.max(c)),
            |a, c| if a >= c { Some(a) } else { None },
            |a, b| if a >= b { Some(a) } else { None },
            "MaxOf",
        );
        // MaxOf(3, B, 7): max(B, 7) = 3, impossible since max must be >= 7
        let args = args_from("REQUEST(MaxOf(3, B, 7))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Contradiction => {}
            other => panic!("expected contradiction, got: {other:?}"),
        }
    }

    #[test]
    fn maxof_contradicts_when_a_lt_b_solving_for_c() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b.max(c)),
            |a, c| if a >= c { Some(a) } else { None },
            |a, b| if a >= b { Some(a) } else { None },
            "MaxOf",
        );
        // MaxOf(3, 7, C): max(7, C) = 3, impossible since max must be >= 7
        let args = args_from("REQUEST(MaxOf(3, 7, C))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Contradiction => {}
            other => panic!("expected contradiction, got: {other:?}"),
        }
    }

    #[test]
    fn copy_maxof_matches_and_binds() {
        let src = crate::types::PodRef(test_helpers::root("s"));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::MaxOf(7.into(), 3.into(), 7.into()), src)
            .build();
        let mut store = ConstraintStore::default();
        let handler = CopyMaxOfHandler;
        // Match first two, bind third
        let args = args_from("REQUEST(MaxOf(7, 3, Z))");

        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 0 && *v == Value::from(7))));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
