use pod2::middleware::{NativePredicate, StatementTmplArg, Value};
use tracing::trace;

use super::sumof::{classify_num, NumArg};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::{Choice, PropagatorResult},
    types::{ConstraintStore, OpTag},
};

/// MaxOf from literals/entries: supports all-ground validation and two-of-three binding/enumeration.
/// Semantics: a = max(b, c)
pub struct MaxOfFromEntriesHandler;

impl OpHandler for MaxOfFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 3 {
            return PropagatorResult::Contradiction;
        }
        trace!("MaxOf: start args_len=3");
        let a = classify_num(&args[0], store, edb);
        let b = classify_num(&args[1], store, edb);
        let c = classify_num(&args[2], store, edb);
        trace!("MaxOf: classified A=? B=? C=?");

        // Type errors or missing facts on bound AKs fail this op path
        match (&a, &b, &c) {
            (NumArg::TypeError, _, _) | (_, NumArg::TypeError, _) | (_, _, NumArg::TypeError) => {
                trace!("MaxOf: type error -> contradiction");
                return PropagatorResult::Contradiction;
            }
            (NumArg::NoFact, _, _) | (_, NumArg::NoFact, _) | (_, _, NumArg::NoFact) => {
                trace!("MaxOf: no fact for bound AK -> contradiction");
                return PropagatorResult::Contradiction;
            }
            _ => {}
        }

        // Collect data
        let mut grounds: Vec<(i64, Vec<(pod2::middleware::Statement, OpTag)>)> = Vec::new();
        let mut akvars: Vec<(usize, pod2::middleware::Key)> = Vec::new();
        let mut waits: Vec<usize> = Vec::new();
        for x in [&a, &b, &c] {
            match x {
                NumArg::Ground { i, premises } => grounds.push((*i, premises.clone())),
                NumArg::AkVar { wc_index, key } => akvars.push((*wc_index, key.clone())),
                NumArg::Wait(w) => {
                    if !store.bindings.contains_key(w) {
                        waits.push(*w)
                    }
                }
                _ => {}
            }
        }
        trace!(grounds = grounds.len(), waits = ?waits, akvars = ?akvars, "MaxOf: classified counts");
        if grounds.len() < 2 {
            waits.sort();
            waits.dedup();
            trace!(?waits, "MaxOf: suspending (insufficient grounds)");
            return if waits.is_empty() {
                PropagatorResult::Contradiction
            } else {
                PropagatorResult::Suspend { on: waits }
            };
        }

        // All ground: validate A == max(B, C)
        if grounds.len() == 3 {
            let a0 = if let NumArg::Ground { i, premises } = a {
                (i, premises)
            } else {
                unreachable!()
            };
            let b0 = if let NumArg::Ground { i, premises } = b {
                (i, premises)
            } else {
                unreachable!()
            };
            let c0 = if let NumArg::Ground { i, premises } = c {
                (i, premises)
            } else {
                unreachable!()
            };
            let max_bc = std::cmp::max(b0.0, c0.0);
            if a0.0 == max_bc {
                trace!(
                    a = a0.0,
                    b = b0.0,
                    c = c0.0,
                    max_bc,
                    "MaxOf: all ground entailed"
                );
                let mut premises = Vec::new();
                premises.extend(a0.1);
                premises.extend(b0.1);
                premises.extend(c0.1);
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
                trace!(
                    a = a0.0,
                    b = b0.0,
                    c = c0.0,
                    max_bc,
                    "MaxOf: all ground mismatch -> contradiction"
                );
                PropagatorResult::Contradiction
            }
        } else {
            // Two-of-three binding
            let mk_ent_bind =
                |wc_index: usize, val: i64, premises: Vec<(pod2::middleware::Statement, OpTag)>| {
                    if premises.is_empty() {
                        PropagatorResult::Entailed {
                            bindings: vec![(wc_index, Value::from(val))],
                            op_tag: OpTag::FromLiterals,
                        }
                    } else {
                        PropagatorResult::Entailed {
                            bindings: vec![(wc_index, Value::from(val))],
                            op_tag: OpTag::Derived { premises },
                        }
                    }
                };

            match (&a, &b, &c) {
                // Unknown is A: A = max(B, C)
                (
                    x,
                    NumArg::Ground {
                        i: bi,
                        premises: pb,
                    },
                    NumArg::Ground {
                        i: ci,
                        premises: pc,
                    },
                ) => {
                    let target = std::cmp::max(*bi, *ci);
                    trace!(b = bi, c = ci, target, "MaxOf: solving A = max(B, C)");
                    match x {
                        NumArg::Wait(w) => mk_ent_bind(*w, target, {
                            let mut p = pb.clone();
                            p.extend(pc.clone());
                            p
                        }),
                        NumArg::AkVar { wc_index, key } => {
                            let choices = crate::util::enumerate_choices_for(
                                key,
                                &Value::from(target),
                                *wc_index,
                                edb,
                            );
                            if choices.is_empty() {
                                PropagatorResult::Contradiction
                            } else {
                                PropagatorResult::Choices {
                                    alternatives: choices,
                                }
                            }
                        }
                        _ => PropagatorResult::Contradiction,
                    }
                }
                // Unknown is B: B can be A (if A >= C) or any value <= A
                // For simplicity, we only allow B = A when A >= C, else contradiction
                (
                    NumArg::Ground {
                        i: ai,
                        premises: pa,
                    },
                    x,
                    NumArg::Ground {
                        i: ci,
                        premises: pc,
                    },
                ) => {
                    if *ai < *ci {
                        // If A < C, then max(B, C) = C, but we need A = max(B, C), contradiction
                        trace!(
                            a = ai,
                            c = ci,
                            "MaxOf: A < C, cannot solve for B -> contradiction"
                        );
                        return PropagatorResult::Contradiction;
                    }
                    // A >= C, so B can be A (then max(A, C) = A) or any value <= A such that max(B, C) = A
                    // For deterministic behavior, we choose B = A
                    let target = *ai;
                    trace!(a = ai, c = ci, target, "MaxOf: solving B = A (A >= C)");
                    match x {
                        NumArg::Wait(w) => mk_ent_bind(*w, target, {
                            let mut p = pa.clone();
                            p.extend(pc.clone());
                            p
                        }),
                        NumArg::AkVar { wc_index, key } => {
                            let choices = crate::util::enumerate_choices_for(
                                key,
                                &Value::from(target),
                                *wc_index,
                                edb,
                            );
                            if choices.is_empty() {
                                PropagatorResult::Contradiction
                            } else {
                                PropagatorResult::Choices {
                                    alternatives: choices,
                                }
                            }
                        }
                        _ => PropagatorResult::Contradiction,
                    }
                }
                // Unknown is C: C can be A (if A >= B) or any value <= A
                // For simplicity, we only allow C = A when A >= B, else contradiction
                (
                    NumArg::Ground {
                        i: ai,
                        premises: pa,
                    },
                    NumArg::Ground {
                        i: bi,
                        premises: pb,
                    },
                    x,
                ) => {
                    if *ai < *bi {
                        // If A < B, then max(B, C) = B, but we need A = max(B, C), contradiction
                        trace!(
                            a = ai,
                            b = bi,
                            "MaxOf: A < B, cannot solve for C -> contradiction"
                        );
                        return PropagatorResult::Contradiction;
                    }
                    // A >= B, so C can be A (then max(B, A) = A) or any value <= A such that max(B, C) = A
                    // For deterministic behavior, we choose C = A
                    let target = *ai;
                    trace!(a = ai, b = bi, target, "MaxOf: solving C = A (A >= B)");
                    match x {
                        NumArg::Wait(w) => mk_ent_bind(*w, target, {
                            let mut p = pa.clone();
                            p.extend(pb.clone());
                            p
                        }),
                        NumArg::AkVar { wc_index, key } => {
                            let choices = crate::util::enumerate_choices_for(
                                key,
                                &Value::from(target),
                                *wc_index,
                                edb,
                            );
                            if choices.is_empty() {
                                PropagatorResult::Contradiction
                            } else {
                                PropagatorResult::Choices {
                                    alternatives: choices,
                                }
                            }
                        }
                        _ => PropagatorResult::Contradiction,
                    }
                }
                _ => PropagatorResult::Contradiction,
            }
        }
    }
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
        let rows = crate::util::ternary_view(
            edb,
            crate::edb::TernaryPred::MaxOf,
            crate::edb::ArgSel::Val,
            crate::edb::ArgSel::Val,
            crate::edb::ArgSel::Val,
        );
        let mut choices: Vec<Choice> = Vec::new();
        for row in rows.into_iter() {
            let mut binds: Vec<(usize, Value)> = Vec::new();
            // Position A
            match &args[0] {
                StatementTmplArg::Wildcard(w) => {
                    if !store.bindings.contains_key(&w.index) {
                        if let Some(v) = row.a.as_literal() {
                            binds.push((w.index, v.clone()));
                        }
                    }
                }
                StatementTmplArg::AnchoredKey(w, _key) => {
                    if !store.bindings.contains_key(&w.index) {
                        if let Some((root, _)) = row.a.as_ak() {
                            binds.push((w.index, Value::from(*root)));
                        }
                    }
                }
                _ => {}
            }
            // Position B
            match &args[1] {
                StatementTmplArg::Wildcard(w) => {
                    if !store.bindings.contains_key(&w.index) {
                        if let Some(v) = row.b.as_literal() {
                            binds.push((w.index, v.clone()));
                        }
                    }
                }
                StatementTmplArg::AnchoredKey(w, _key) => {
                    if !store.bindings.contains_key(&w.index) {
                        if let Some((root, _)) = row.b.as_ak() {
                            binds.push((w.index, Value::from(*root)));
                        }
                    }
                }
                _ => {}
            }
            // Position C
            match &args[2] {
                StatementTmplArg::Wildcard(w) => {
                    if !store.bindings.contains_key(&w.index) {
                        if let Some(v) = row.c.as_literal() {
                            binds.push((w.index, v.clone()));
                        }
                    }
                }
                StatementTmplArg::AnchoredKey(w, _key) => {
                    if !store.bindings.contains_key(&w.index) {
                        if let Some((root, _)) = row.c.as_ak() {
                            binds.push((w.index, Value::from(*root)));
                        }
                    }
                }
                _ => {}
            }
            choices.push(Choice {
                bindings: binds,
                op_tag: OpTag::CopyStatement { source: row.src },
            });
        }
        if choices.is_empty() {
            let waits = crate::prop::wildcards_in_args(args)
                .into_iter()
                .filter(|i| !store.bindings.contains_key(i))
                .collect::<Vec<_>>();
            if waits.is_empty() {
                PropagatorResult::Contradiction
            } else {
                PropagatorResult::Suspend { on: waits }
            }
        } else {
            PropagatorResult::Choices {
                alternatives: choices,
            }
        }
    }
}

pub fn register_maxof_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(NativePredicate::MaxOf, Box::new(MaxOfFromEntriesHandler));
    reg.register(NativePredicate::MaxOf, Box::new(CopyMaxOfHandler));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        edb::MockEdbView,
        test_helpers::{self, args_from},
        types::ConstraintStore,
    };

    #[test]
    fn maxof_two_of_three_binds_wildcard() {
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = MaxOfFromEntriesHandler;
        let args = args_from("REQUEST(MaxOf(?X, 3, 7))");
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
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = MaxOfFromEntriesHandler;
        let args = args_from("REQUEST(MaxOf(7, 3, 7))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { .. } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn maxof_all_ground_mismatch_contradicts() {
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = MaxOfFromEntriesHandler;
        let args = args_from("REQUEST(MaxOf(5, 3, 7))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Contradiction => {}
            other => panic!("expected contradiction, got: {other:?}"),
        }
    }

    #[test]
    fn maxof_solves_b_when_a_ge_c() {
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = MaxOfFromEntriesHandler;
        // MaxOf(7, ?B, 3): max(B, 3) = 7, so B = 7
        let args = args_from("REQUEST(MaxOf(7, ?B, 3))");
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
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = MaxOfFromEntriesHandler;
        // MaxOf(7, 3, ?C): max(3, C) = 7, so C = 7
        let args = args_from("REQUEST(MaxOf(7, 3, ?C))");
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
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = MaxOfFromEntriesHandler;
        // MaxOf(3, ?B, 7): max(B, 7) = 3, impossible since max must be >= 7
        let args = args_from("REQUEST(MaxOf(3, ?B, 7))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Contradiction => {}
            other => panic!("expected contradiction, got: {other:?}"),
        }
    }

    #[test]
    fn maxof_contradicts_when_a_lt_b_solving_for_c() {
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = MaxOfFromEntriesHandler;
        // MaxOf(3, 7, ?C): max(7, C) = 3, impossible since max must be >= 7
        let args = args_from("REQUEST(MaxOf(3, 7, ?C))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Contradiction => {}
            other => panic!("expected contradiction, got: {other:?}"),
        }
    }

    #[test]
    fn copy_maxof_matches_and_binds() {
        let mut edb = MockEdbView::default();
        let src = crate::types::PodRef(test_helpers::root("s"));
        edb.add_max_row_vals(Value::from(7), Value::from(3), Value::from(7), src);
        let mut store = ConstraintStore::default();
        let handler = CopyMaxOfHandler;
        // Match first two, bind third
        let args = args_from("REQUEST(MaxOf(7, 3, ?Z))");

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
