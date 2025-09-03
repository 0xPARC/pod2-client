use pod2::middleware::StatementTmplArg;
use tracing::trace;

use super::util::{classify_num, NumArg};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
};

pub struct BinaryComparisonHandler {
    op: fn(i64, i64) -> bool,
    op_name: &'static str,
}

impl BinaryComparisonHandler {
    pub fn new(op: fn(i64, i64) -> bool, op_name: &'static str) -> Self {
        Self { op, op_name }
    }
}

impl OpHandler for BinaryComparisonHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }
        trace!(op = self.op_name, args = ?args, "BinaryComparison: start");

        let a0 = classify_num(&args[0], store, edb);
        let a1 = classify_num(&args[1], store, edb);

        match (&a0, &a1) {
            (NumArg::TypeError, _) | (_, NumArg::TypeError) => {
                return PropagatorResult::Contradiction
            }
            (NumArg::NoFact, _) | (_, NumArg::NoFact) => return PropagatorResult::Contradiction,
            _ => {}
        }

        let mut waits: Vec<usize> = vec![];
        if let NumArg::Wait(w) = a0 {
            if !store.bindings.contains_key(&w) {
                waits.push(w);
            }
        }
        if let NumArg::Wait(w) = a1 {
            if !store.bindings.contains_key(&w) {
                waits.push(w);
            }
        }
        if !waits.is_empty() {
            waits.sort();
            waits.dedup();
            return PropagatorResult::Suspend { on: waits };
        }

        let (i0, prem0) = match a0 {
            NumArg::Ground { i, premises } => (i, premises),
            _ => unreachable!(),
        };

        let (i1, prem1) = match a1 {
            NumArg::Ground { i, premises } => (i, premises),
            _ => unreachable!(),
        };

        if (self.op)(i0, i1) {
            let mut premises = Vec::new();
            premises.extend(prem0);
            premises.extend(prem1);
            if premises.is_empty() {
                PropagatorResult::Entailed {
                    bindings: vec![],
                    op_tag: OpTag::FromLiterals,
                }
            } else {
                PropagatorResult::Entailed {
                    bindings: vec![],
                    op_tag: OpTag::Derived { premises },
                }
            }
        } else {
            PropagatorResult::Contradiction
        }
    }
}
