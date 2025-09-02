use pod2::middleware::{NativePredicate, StatementTmplArg, Value};
use tracing::trace;

use super::sumof::{classify_num, NumArg};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::{Choice, PropagatorResult},
    types::{ConstraintStore, OpTag},
};

/// ProductOf from literals/entries: supports all-ground validation and two-of-three binding/enumeration.
/// Semantics: a = b * c
pub struct ProductOfFromEntriesHandler;

impl OpHandler for ProductOfFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 3 {
            return PropagatorResult::Contradiction;
        }
        trace!("ProductOf: start args_len=3");
        let a = classify_num(&args[0], store, edb);
        let b = classify_num(&args[1], store, edb);
        let c = classify_num(&args[2], store, edb);
        trace!("ProductOf: classified A=? B=? C=?");

        // Type errors or missing facts on bound AKs fail this op path
        match (&a, &b, &c) {
            (NumArg::TypeError, _, _) | (_, NumArg::TypeError, _) | (_, _, NumArg::TypeError) => {
                trace!("ProductOf: type error -> contradiction");
                return PropagatorResult::Contradiction;
            }
            (NumArg::NoFact, _, _) | (_, NumArg::NoFact, _) | (_, _, NumArg::NoFact) => {
                trace!("ProductOf: no fact for bound AK -> contradiction");
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
        if grounds.len() < 2 {
            waits.sort();
            waits.dedup();
            trace!(?waits, "ProductOf: suspending (insufficient grounds)");
            return if waits.is_empty() {
                PropagatorResult::Contradiction
            } else {
                PropagatorResult::Suspend { on: waits }
            };
        }

        // All ground: validate A == B * C
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
            if a0.0 == b0.0 * c0.0 {
                trace!(
                    a = a0.0,
                    b = b0.0,
                    c = c0.0,
                    "ProductOf: all ground entailed"
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
                    "ProductOf: all ground mismatch -> contradiction"
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
                // Unknown is C: C = A / B with integer arithmetic. Only allow exact division.
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
                    if *bi == 0 || ai.rem_euclid(*bi) != 0 {
                        return PropagatorResult::Contradiction;
                    }
                    let target = ai / bi;
                    trace!(a = ai, b = bi, target, "ProductOf: solving C = A / B");
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
                // Unknown is B: B = A / C
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
                    if *ci == 0 || ai.rem_euclid(*ci) != 0 {
                        return PropagatorResult::Contradiction;
                    }
                    let target = ai / ci;
                    trace!(a = ai, c = ci, target, "ProductOf: solving B = A / C");
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
                // Unknown is A: A = B * C
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
                    let target = bi * ci;
                    trace!(b = bi, c = ci, target, "ProductOf: solving A = B * C");
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
                _ => PropagatorResult::Contradiction,
            }
        }
    }
}

pub fn register_productof_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(
        NativePredicate::ProductOf,
        Box::new(ProductOfFromEntriesHandler),
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
        let rows = crate::util::ternary_view(
            edb,
            crate::edb::TernaryPred::ProductOf,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{edb::MockEdbView, test_helpers::args_from, types::ConstraintStore};

    #[test]
    fn productof_two_of_three_binds_wildcard() {
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = ProductOfFromEntriesHandler;
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
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = ProductOfFromEntriesHandler;
        let args = args_from("REQUEST(ProductOf(12, 3, 4))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { .. } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn productof_division_requires_exact() {
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = ProductOfFromEntriesHandler;
        let args = args_from("REQUEST(ProductOf(7, 3, ?Z))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Contradiction => {}
            _ => panic!("expected contradiction for non-exact division case"),
        }
    }
}
