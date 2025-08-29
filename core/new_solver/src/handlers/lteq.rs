use pod2::middleware::{Hash, NativePredicate, Statement, StatementTmplArg, Value};

use crate::{
    edb::{ArgSel, BinaryPred, EdbView},
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
    util::{binary_view, contains_stmt},
};

/// Value-centric LtEqFromEntries: resolve ints from literals, wildcards, or AKs; suspend if unknown.
pub struct LtEqFromEntriesHandler;

impl OpHandler for LtEqFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }

        enum ArgVal {
            Ground {
                i: i64,
                premises: Vec<(Statement, OpTag)>,
            },
            Wait(usize),
            TypeError,
            NoFact,
        }

        fn int_from_value(v: &Value) -> Result<i64, ()> {
            i64::try_from(v.typed()).map_err(|_| ())
        }

        fn classify(a: &StatementTmplArg, store: &ConstraintStore, edb: &dyn EdbView) -> ArgVal {
            match a {
                StatementTmplArg::Literal(v) => match int_from_value(v) {
                    Ok(i) => ArgVal::Ground {
                        i,
                        premises: vec![],
                    },
                    Err(_) => ArgVal::TypeError,
                },
                StatementTmplArg::Wildcard(w) => match store.bindings.get(&w.index) {
                    Some(v) => match int_from_value(v) {
                        Ok(i) => ArgVal::Ground {
                            i,
                            premises: vec![],
                        },
                        Err(_) => ArgVal::TypeError,
                    },
                    None => ArgVal::Wait(w.index),
                },
                StatementTmplArg::AnchoredKey(w, key) => match store.bindings.get(&w.index) {
                    Some(bound_root_val) => {
                        let root: Hash = Hash::from(bound_root_val.raw());
                        if let Some(val) = edb.contains_value(&root, key) {
                            if let Ok(i) = int_from_value(&val) {
                                let tag = crate::util::tag_from_source(
                                    key,
                                    &val,
                                    match edb.contains_source(&root, key, &val) {
                                        Some(src) => src,
                                        None => return ArgVal::NoFact,
                                    },
                                );
                                let c = contains_stmt(root, key, val);
                                ArgVal::Ground {
                                    i,
                                    premises: vec![(c, tag)],
                                }
                            } else {
                                ArgVal::TypeError
                            }
                        } else {
                            ArgVal::NoFact
                        }
                    }
                    None => ArgVal::Wait(w.index),
                },
                _ => ArgVal::TypeError,
            }
        }

        let a0 = classify(&args[0], store, edb);
        let a1 = classify(&args[1], store, edb);

        match (&a0, &a1) {
            (ArgVal::TypeError, _) | (_, ArgVal::TypeError) => {
                return PropagatorResult::Contradiction
            }
            (ArgVal::NoFact, _) | (_, ArgVal::NoFact) => return PropagatorResult::Contradiction,
            _ => {}
        }

        let mut waits: Vec<usize> = vec![];
        if let ArgVal::Wait(w) = a0 {
            if !store.bindings.contains_key(&w) {
                waits.push(w)
            }
        }
        if let ArgVal::Wait(w) = a1 {
            if !store.bindings.contains_key(&w) {
                waits.push(w)
            }
        }
        if !waits.is_empty() {
            waits.sort();
            waits.dedup();
            return PropagatorResult::Suspend { on: waits };
        }

        let (i0, prem0) = match a0 {
            ArgVal::Ground { i, premises } => (i, premises),
            _ => unreachable!(),
        };
        let (i1, prem1) = match a1 {
            ArgVal::Ground { i, premises } => (i, premises),
            _ => unreachable!(),
        };

        if i0 <= i1 {
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
        // copy path uses binary_view; no direct Statement matching
        let left = &args[0];
        let right = &args[1];
        let mut choices: Vec<crate::prop::Choice> = Vec::new();
        match (left, right) {
            (StatementTmplArg::Wildcard(wl), StatementTmplArg::Wildcard(wr)) => {
                if !store.bindings.contains_key(&wl.index)
                    && !store.bindings.contains_key(&wr.index)
                {
                    for row in binary_view(edb, BinaryPred::LtEq, ArgSel::Val, ArgSel::Val) {
                        if let (Some(l), Some(r)) = (row.left.as_literal(), row.right.as_literal())
                        {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wl.index, l.clone()), (wr.index, r.clone())],
                                op_tag: OpTag::CopyStatement { source: row.src },
                            });
                        }
                    }
                }
                if let Some(vl) = store.bindings.get(&wl.index) {
                    for row in binary_view(edb, BinaryPred::LtEq, ArgSel::Literal(vl), ArgSel::Val)
                    {
                        if let Some(r) = row.right.as_literal() {
                            if !store.bindings.contains_key(&wr.index) {
                                choices.push(crate::prop::Choice {
                                    bindings: vec![(wr.index, r.clone())],
                                    op_tag: OpTag::CopyStatement { source: row.src },
                                });
                            }
                        }
                    }
                }
                if let Some(vr) = store.bindings.get(&wr.index) {
                    for row in binary_view(edb, BinaryPred::LtEq, ArgSel::Val, ArgSel::Literal(vr))
                    {
                        if let Some(l) = row.left.as_literal() {
                            if !store.bindings.contains_key(&wl.index) {
                                choices.push(crate::prop::Choice {
                                    bindings: vec![(wl.index, l.clone())],
                                    op_tag: OpTag::CopyStatement { source: row.src },
                                });
                            }
                        }
                    }
                }
            }
            (StatementTmplArg::Literal(vl), StatementTmplArg::Wildcard(wr)) => {
                for row in binary_view(edb, BinaryPred::LtEq, ArgSel::Literal(vl), ArgSel::Val) {
                    if let Some(vr) = row.right.as_literal() {
                        choices.push(crate::prop::Choice {
                            bindings: vec![(wr.index, vr.clone())],
                            op_tag: OpTag::CopyStatement { source: row.src },
                        });
                    }
                }
            }
            (StatementTmplArg::Wildcard(wl), StatementTmplArg::Literal(vr)) => {
                for row in binary_view(edb, BinaryPred::LtEq, ArgSel::Val, ArgSel::Literal(vr)) {
                    if let Some(vl) = row.left.as_literal() {
                        choices.push(crate::prop::Choice {
                            bindings: vec![(wl.index, vl.clone())],
                            op_tag: OpTag::CopyStatement { source: row.src },
                        });
                    }
                }
            }
            (StatementTmplArg::AnchoredKey(wc_l, key_l), StatementTmplArg::Literal(val_r)) => {
                for v in binary_view(
                    edb,
                    BinaryPred::LtEq,
                    ArgSel::AkByKey(key_l),
                    ArgSel::Literal(val_r),
                ) {
                    if let Some((root, _)) = v.left.as_ak() {
                        choices.push(crate::prop::Choice {
                            bindings: vec![(wc_l.index, Value::from(*root))],
                            op_tag: OpTag::CopyStatement { source: v.src },
                        });
                    }
                }
            }
            (StatementTmplArg::AnchoredKey(wc_l, key_l), StatementTmplArg::Wildcard(wv)) => {
                // If right wildcard is bound to a literal value, treat as literal and bind left root
                if let Some(vr_lit) = store.bindings.get(&wv.index) {
                    for v in binary_view(
                        edb,
                        BinaryPred::LtEq,
                        ArgSel::AkByKey(key_l),
                        ArgSel::Literal(vr_lit),
                    ) {
                        if let Some((root, _)) = v.left.as_ak() {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wc_l.index, Value::from(*root))],
                                op_tag: OpTag::CopyStatement { source: v.src },
                            });
                        }
                    }
                } else if let Some(root) =
                    store.bindings.get(&wc_l.index).map(|v| Hash::from(v.raw()))
                {
                    // If left root is bound, enumerate RHS values and bind right wildcard
                    for row in binary_view(
                        edb,
                        BinaryPred::LtEq,
                        ArgSel::AkExact {
                            root: &root,
                            key: key_l,
                        },
                        ArgSel::Val,
                    ) {
                        if let Some(vr) = row.right.as_literal() {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wv.index, vr.clone())],
                                op_tag: OpTag::CopyStatement { source: row.src },
                            });
                        }
                    }
                }
            }
            (StatementTmplArg::Literal(val_l), StatementTmplArg::AnchoredKey(wc_r, key_r)) => {
                for v in binary_view(
                    edb,
                    BinaryPred::LtEq,
                    ArgSel::Literal(val_l),
                    ArgSel::AkByKey(key_r),
                ) {
                    if let Some((root, _)) = v.right.as_ak() {
                        choices.push(crate::prop::Choice {
                            bindings: vec![(wc_r.index, Value::from(*root))],
                            op_tag: OpTag::CopyStatement { source: v.src },
                        });
                    }
                }
            }
            (StatementTmplArg::Wildcard(wv), StatementTmplArg::AnchoredKey(wc_r, key_r)) => {
                // If left wildcard is bound to a literal value, treat as literal and bind right root
                if let Some(vl_lit) = store.bindings.get(&wv.index) {
                    for v in binary_view(
                        edb,
                        BinaryPred::LtEq,
                        ArgSel::Literal(vl_lit),
                        ArgSel::AkByKey(key_r),
                    ) {
                        if let Some((root, _)) = v.right.as_ak() {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wc_r.index, Value::from(*root))],
                                op_tag: OpTag::CopyStatement { source: v.src },
                            });
                        }
                    }
                } else if let Some(root) =
                    store.bindings.get(&wc_r.index).map(|v| Hash::from(v.raw()))
                {
                    // If right root is bound, enumerate LHS values and bind left wildcard
                    for row in binary_view(
                        edb,
                        BinaryPred::LtEq,
                        ArgSel::Val,
                        ArgSel::AkExact {
                            root: &root,
                            key: key_r,
                        },
                    ) {
                        if let Some(vl) = row.left.as_literal() {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wv.index, vl.clone())],
                                op_tag: OpTag::CopyStatement { source: row.src },
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        if choices.is_empty() {
            let waits_all = crate::prop::wildcards_in_args(args);
            let waits: Vec<_> = waits_all
                .into_iter()
                .filter(|i| !store.bindings.contains_key(i))
                .collect();
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

pub fn register_lteq_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(NativePredicate::LtEq, Box::new(LtEqFromEntriesHandler));
    reg.register(NativePredicate::LtEq, Box::new(CopyLtEqHandler));
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{containers::Dictionary, Params, StatementTmplArg};

    use super::*;
    use crate::{edb::MockEdbView, test_helpers, types::ConstraintStore, OpTag};

    fn args_from(query: &str) -> Vec<StatementTmplArg> {
        let tmpl = crate::test_helpers::parse_first_tmpl(query);
        tmpl.args().to_vec()
    }

    #[test]
    fn lteq_from_entries_literals() {
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = LtEqFromEntriesHandler;
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
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(7))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        edb.add_full_dict(dict);
        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(root));
        let handler = LtEqFromEntriesHandler;
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
        let mut edb = MockEdbView::default();
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
        edb.add_full_dict(dl);
        edb.add_full_dict(dr);

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(rl));
        store.bindings.insert(1, Value::from(rr));

        let handler = LtEqFromEntriesHandler;
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
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = LtEqFromEntriesHandler;
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
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = LtEqFromEntriesHandler;
        let args = args_from("REQUEST(LtEq(\"foo\", 5))");
        let res = handler.propagate(&args, &mut store, &edb);
        assert!(matches!(res, PropagatorResult::Contradiction));
    }

    #[test]
    fn copy_lteq_binds_both_from_vv_fact() {
        let mut edb = MockEdbView::default();
        let src = crate::types::PodRef(test_helpers::root("s"));
        edb.add_lte_row_vals(Value::from(3), Value::from(5), src);

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
        let mut edb = MockEdbView::default();
        let src = crate::types::PodRef(test_helpers::root("s"));
        edb.add_lte_row_vals(Value::from(3), Value::from(5), src);

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
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(10))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = crate::types::PodRef(r);
        edb.add_lte_row_lak_rval(r, test_helpers::key("k"), Value::from(10), src);

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
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(10))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = crate::types::PodRef(r);
        edb.add_lte_row_lval_rak(Value::from(5), r, test_helpers::key("k"), src);

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
