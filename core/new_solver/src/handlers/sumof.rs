use pod2::middleware::{
    AnchoredKey, Hash, Key, NativePredicate, Statement, StatementTmplArg, Value, ValueRef,
};

use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::{Choice, PropagatorResult},
    types::{ConstraintStore, OpTag},
    util::{contains_stmt, enumerate_choices_for, tag_from_source},
};

/// Helper: classify an argument into a numeric int if possible, with premises when using AKs.
enum NumArg {
    Ground {
        i: i64,
        premises: Vec<(Statement, OpTag)>,
    },
    AkVar {
        wc_index: usize,
        key: Key,
    },
    Wait(usize),
    TypeError,
    NoFact,
}

fn int_from_value(v: &Value) -> Result<i64, ()> {
    i64::try_from(v.typed()).map_err(|_| ())
}

fn classify_num(arg: &StatementTmplArg, store: &ConstraintStore, edb: &dyn EdbView) -> NumArg {
    match arg {
        StatementTmplArg::Literal(v) => match int_from_value(v) {
            Ok(i) => NumArg::Ground {
                i,
                premises: vec![],
            },
            Err(_) => NumArg::TypeError,
        },
        StatementTmplArg::Wildcard(w) => match store.bindings.get(&w.index) {
            Some(v) => match int_from_value(v) {
                Ok(i) => NumArg::Ground {
                    i,
                    premises: vec![],
                },
                Err(_) => NumArg::TypeError,
            },
            None => NumArg::Wait(w.index),
        },
        StatementTmplArg::AnchoredKey(w, key) => match store.bindings.get(&w.index) {
            Some(bound_root_val) => {
                let root: Hash = Hash::from(bound_root_val.raw());
                if let Some(val) = edb.contains_value(&root, key) {
                    if let Ok(i) = int_from_value(&val) {
                        let src = match edb.contains_source(&root, key, &val) {
                            Some(s) => s,
                            None => return NumArg::NoFact,
                        };
                        let tag = tag_from_source(key, &val, src);
                        let c = contains_stmt(root, key, val);
                        NumArg::Ground {
                            i,
                            premises: vec![(c, tag)],
                        }
                    } else {
                        NumArg::TypeError
                    }
                } else {
                    NumArg::NoFact
                }
            }
            None => NumArg::AkVar {
                wc_index: w.index,
                key: key.clone(),
            },
        },
        _ => NumArg::TypeError,
    }
}

/// SumOf from literals/entries: supports all-ground validation and two-of-three binding/enumeration.
pub struct SumOfFromEntriesHandler;

impl OpHandler for SumOfFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 3 {
            return PropagatorResult::Contradiction;
        }
        let a = classify_num(&args[0], store, edb);
        let b = classify_num(&args[1], store, edb);
        let c = classify_num(&args[2], store, edb);

        // Type errors or missing facts on bound AKs fail this op path
        match (&a, &b, &c) {
            (NumArg::TypeError, _, _) | (_, NumArg::TypeError, _) | (_, _, NumArg::TypeError) => {
                return PropagatorResult::Contradiction
            }
            (NumArg::NoFact, _, _) | (_, NumArg::NoFact, _) | (_, _, NumArg::NoFact) => {
                return PropagatorResult::Contradiction
            }
            _ => {}
        }

        // Collect waits if fewer than two resolvable
        let mut grounds: Vec<(i64, Vec<(Statement, OpTag)>)> = Vec::new();
        let mut akvars: Vec<(usize, Key)> = Vec::new();
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
            return if waits.is_empty() {
                PropagatorResult::Contradiction
            } else {
                PropagatorResult::Suspend { on: waits }
            };
        }

        // All ground: validate A == B + C
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
            if a0.0 == b0.0 + c0.0 {
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
                PropagatorResult::Contradiction
            }
        } else {
            // Two-of-three: determine which is unknown
            // Compute target depending on which position is unknown
            // Prefer binding wildcard value directly; if AK var, enumerate choices for root with computed value.
            let mut mk_ent_bind = |wc_index: usize, val: i64, premises: Vec<(Statement, OpTag)>| {
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
                    // Unknown is C: target = A - B
                    let target = ai - bi;
                    match x {
                        NumArg::Wait(w) => mk_ent_bind(*w, target, {
                            let mut p = pa.clone();
                            p.extend(pb.clone());
                            p
                        }),
                        NumArg::AkVar { wc_index, key } => {
                            let choices =
                                enumerate_choices_for(key, &Value::from(target), *wc_index, edb);
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
                    // Unknown is B: target = A - C
                    let target = ai - ci;
                    match x {
                        NumArg::Wait(w) => mk_ent_bind(*w, target, {
                            let mut p = pa.clone();
                            p.extend(pc.clone());
                            p
                        }),
                        NumArg::AkVar { wc_index, key } => {
                            let choices =
                                enumerate_choices_for(key, &Value::from(target), *wc_index, edb);
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
                    // Unknown is A: target = B + C
                    let target = bi + ci;
                    match x {
                        NumArg::Wait(w) => mk_ent_bind(*w, target, {
                            let mut p = pb.clone();
                            p.extend(pc.clone());
                            p
                        }),
                        NumArg::AkVar { wc_index, key } => {
                            let choices =
                                enumerate_choices_for(key, &Value::from(target), *wc_index, edb);
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

/// Copy SumOf rows matching two-of-three syntactically, binding the third when wildcard or AK root wildcard.
pub struct CopySumOfHandler;

fn arg_matches_tmpl(vr: &ValueRef, tmpl: &StatementTmplArg, store: &ConstraintStore) -> bool {
    match (vr, tmpl) {
        (ValueRef::Literal(vs), StatementTmplArg::Literal(vt)) => vs == vt,
        (ValueRef::Literal(vs), StatementTmplArg::Wildcard(w)) => store
            .bindings
            .get(&w.index)
            .map(|bv| bv == vs)
            .unwrap_or(false),
        (
            ValueRef::Key(AnchoredKey { root: rs, key: ks }),
            StatementTmplArg::AnchoredKey(w, kt),
        ) => {
            kt.hash() == ks.hash()
                && store
                    .bindings
                    .get(&w.index)
                    .map(|bv| Hash::from(bv.raw()) == *rs)
                    .unwrap_or(true)
        }
        _ => false,
    }
}

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
        let mut choices: Vec<Choice> = Vec::new();
        for (st, src) in edb.sumof_rows().into_iter() {
            if let Statement::SumOf(a, b, c) = st {
                // Try each position as the "third" to bind
                // Case 1: match a,b; handle c
                if arg_matches_tmpl(&a, &args[0], store) && arg_matches_tmpl(&b, &args[1], store) {
                    match (&c, &args[2]) {
                        (ValueRef::Literal(v), StatementTmplArg::Wildcard(w)) => {
                            choices.push(Choice {
                                bindings: vec![(w.index, v.clone())],
                                op_tag: OpTag::CopyStatement {
                                    source: src.clone(),
                                },
                            });
                        }
                        (ValueRef::Literal(vc), StatementTmplArg::Literal(vt)) if vc == vt => {
                            choices.push(Choice {
                                bindings: vec![],
                                op_tag: OpTag::CopyStatement {
                                    source: src.clone(),
                                },
                            });
                        }
                        (
                            ValueRef::Key(AnchoredKey { root, key }),
                            StatementTmplArg::AnchoredKey(w, kt),
                        ) if kt.hash() == key.hash() => {
                            // Bind root if unbound
                            if !store.bindings.contains_key(&w.index) {
                                choices.push(Choice {
                                    bindings: vec![(w.index, Value::from(*root))],
                                    op_tag: OpTag::CopyStatement {
                                        source: src.clone(),
                                    },
                                });
                            } else {
                                choices.push(Choice {
                                    bindings: vec![],
                                    op_tag: OpTag::CopyStatement {
                                        source: src.clone(),
                                    },
                                });
                            }
                        }
                        _ => {}
                    }
                }
                // Case 2: match a,c; handle b
                if arg_matches_tmpl(&a, &args[0], store) && arg_matches_tmpl(&c, &args[2], store) {
                    match (&b, &args[1]) {
                        (ValueRef::Literal(v), StatementTmplArg::Wildcard(w)) => {
                            choices.push(Choice {
                                bindings: vec![(w.index, v.clone())],
                                op_tag: OpTag::CopyStatement {
                                    source: src.clone(),
                                },
                            })
                        }
                        (ValueRef::Literal(vb), StatementTmplArg::Literal(vt)) if vb == vt => {
                            choices.push(Choice {
                                bindings: vec![],
                                op_tag: OpTag::CopyStatement {
                                    source: src.clone(),
                                },
                            })
                        }
                        (
                            ValueRef::Key(AnchoredKey { root, key }),
                            StatementTmplArg::AnchoredKey(w, kt),
                        ) if kt.hash() == key.hash() => {
                            if !store.bindings.contains_key(&w.index) {
                                choices.push(Choice {
                                    bindings: vec![(w.index, Value::from(*root))],
                                    op_tag: OpTag::CopyStatement {
                                        source: src.clone(),
                                    },
                                });
                            } else {
                                choices.push(Choice {
                                    bindings: vec![],
                                    op_tag: OpTag::CopyStatement {
                                        source: src.clone(),
                                    },
                                });
                            }
                        }
                        _ => {}
                    }
                }
                // Case 3: match b,c; handle a
                if arg_matches_tmpl(&b, &args[1], store) && arg_matches_tmpl(&c, &args[2], store) {
                    match (&a, &args[0]) {
                        (ValueRef::Literal(v), StatementTmplArg::Wildcard(w)) => {
                            choices.push(Choice {
                                bindings: vec![(w.index, v.clone())],
                                op_tag: OpTag::CopyStatement {
                                    source: src.clone(),
                                },
                            })
                        }
                        (ValueRef::Literal(va), StatementTmplArg::Literal(vt)) if va == vt => {
                            choices.push(Choice {
                                bindings: vec![],
                                op_tag: OpTag::CopyStatement {
                                    source: src.clone(),
                                },
                            })
                        }
                        (
                            ValueRef::Key(AnchoredKey { root, key }),
                            StatementTmplArg::AnchoredKey(w, kt),
                        ) if kt.hash() == key.hash() => {
                            if !store.bindings.contains_key(&w.index) {
                                choices.push(Choice {
                                    bindings: vec![(w.index, Value::from(*root))],
                                    op_tag: OpTag::CopyStatement {
                                        source: src.clone(),
                                    },
                                });
                            } else {
                                choices.push(Choice {
                                    bindings: vec![],
                                    op_tag: OpTag::CopyStatement {
                                        source: src.clone(),
                                    },
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if choices.is_empty() {
            // Suspend when only one argument is concretely matched? Fallback to suspend on wildcards referenced
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

pub fn register_sumof_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(NativePredicate::SumOf, Box::new(SumOfFromEntriesHandler));
    reg.register(NativePredicate::SumOf, Box::new(CopySumOfHandler));
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{containers::Dictionary, Params};

    use super::*;
    use crate::{edb::MockEdbView, test_helpers, types::ConstraintStore};

    fn args_from(query: &str) -> Vec<StatementTmplArg> {
        let tmpl = test_helpers::parse_first_tmpl(query);
        tmpl.args().to_vec()
    }

    #[test]
    fn sumof_two_of_three_binds_wildcard() {
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = SumOfFromEntriesHandler;
        let args = args_from("REQUEST(SumOf(?X, 3, 4))");
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
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("a"), Value::from(7))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        edb.add_full_dict(dict);
        let mut store = ConstraintStore::default();
        let handler = SumOfFromEntriesHandler;
        let args = args_from("REQUEST(SumOf(?R[\"a\"], 3, 4))");
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
        let mut edb = MockEdbView::default();
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
        edb.add_full_dict(d1);
        edb.add_full_dict(d2);
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(r1));
        store.bindings.insert(1, Value::from(r2));
        let handler = SumOfFromEntriesHandler;
        let args = args_from("REQUEST(SumOf(7, ?A[\"x\"], ?B[\"y\"]))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => match op_tag {
                OpTag::Derived { premises } => assert_eq!(premises.len(), 2),
                other => panic!("unexpected tag: {other:?}"),
            },
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn copy_sumof_matches_two_of_three_and_binds_third() {
        let mut edb = MockEdbView::default();
        let src = crate::types::PodRef(test_helpers::root("s"));
        edb.add_sum_row_vals(Value::from(15), Value::from(5), Value::from(10), src);
        let mut store = ConstraintStore::default();
        let handler = CopySumOfHandler;
        // Match first two, bind third
        let args = args_from("REQUEST(SumOf(15, 5, ?Z))");
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
