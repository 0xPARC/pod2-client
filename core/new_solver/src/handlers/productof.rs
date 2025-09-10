use pod2::middleware::{NativePredicate, StatementTmplArg};

use super::{
    ternary::TernaryArithmeticHandler,
    util::{arg_to_selector, handle_copy_results},
};
use crate::{edb::EdbView, op::OpHandler, prop::PropagatorResult, types::ConstraintStore};

pub fn register_productof_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::ProductOf,
        Box::new(TernaryArithmeticHandler::new(
            |b, c| Some(b * c),
            |a, c| if c != 0 { a.checked_div(c) } else { None },
            |a, b| if b != 0 { a.checked_div(b) } else { None },
            "ProductOf",
        )),
    );
    reg.register(NativePredicate::ProductOf, Box::new(CopyProductOfHandler));
}

/// CopyProductOf: copy rows from EDB: matches any two-of-three and binds the third.
pub struct CopyProductOfHandler;

impl OpHandler for CopyProductOfHandler {
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
            crate::edb::PredicateKey::Native(NativePredicate::ProductOf),
            &[sel_a, sel_b, sel_c],
        );

        handle_copy_results(results, args, store)
    }
}

#[cfg(test)]
mod tests {
    use pod2::middleware::Value;

    use super::*;
    use crate::{edb::ImmutableEdbBuilder, test_helpers::args_from, types::ConstraintStore};

    #[test]
    fn productof_two_of_three_binds_wildcard() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b * c),
            |a, c| if c != 0 { a.checked_div(c) } else { None },
            |a, b| if b != 0 { a.checked_div(b) } else { None },
            "ProductOf",
        );
        let args = args_from("REQUEST(ProductOf(?X, 3, 4))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { bindings, .. } => {
                assert_eq!(bindings, vec![(0, Value::from(12))]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn productof_all_ground_validates() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b * c),
            |a, c| if c != 0 { a.checked_div(c) } else { None },
            |a, b| if b != 0 { a.checked_div(b) } else { None },
            "ProductOf",
        );
        let args = args_from("REQUEST(ProductOf(12, 3, 4))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { .. } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn productof_division_requires_exact() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = TernaryArithmeticHandler::new(
            |b, c| Some(b * c),
            |a, c| {
                if c != 0 {
                    let b = a.checked_div(c);
                    b.filter(|&b| b * c == a)
                } else {
                    None
                }
            },
            |a, b| {
                if b != 0 {
                    let c = a.checked_div(b);
                    c.filter(|&c| c * b == a)
                } else {
                    None
                }
            },
            "ProductOf",
        );
        let args = args_from("REQUEST(ProductOf(7, 3, ?Z))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Contradiction => {}
            _ => panic!("expected contradiction for non-exact division case"),
        }
    }
}
