use pod2::middleware::{Hash, NativePredicate, Statement, StatementTmplArg, Value};
use tracing::trace;

use crate::{
    edb::{ArgSel, BinaryPred, EdbView},
    op::OpHandler,
    prop::PropagatorResult,
    types::{ConstraintStore, OpTag},
    util::{binary_view, contains_stmt},
};

/// Value-centric LtFromEntries: resolve ints from literals, wildcards, or AKs; suspend if unknown.
pub struct LtFromEntriesHandler;

impl OpHandler for LtFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }
        trace!(args = ?args, "Lt: start");

        // Classify an argument into an integer value if possible, along with any premises
        // (Contains facts) required to justify AK value extraction.
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
        // Lightweight classification summary for tracing
        let mut kind0 = "";
        let mut kind1 = "";
        match &a0 {
            ArgVal::Ground { i, .. } => kind0 = Box::leak(format!("ground({i})").into_boxed_str()),
            ArgVal::Wait(w) => kind0 = Box::leak(format!("wait({w})").into_boxed_str()),
            ArgVal::TypeError => kind0 = "type_error",
            ArgVal::NoFact => kind0 = "no_fact",
        }
        match &a1 {
            ArgVal::Ground { i, .. } => kind1 = Box::leak(format!("ground({i})").into_boxed_str()),
            ArgVal::Wait(w) => kind1 = Box::leak(format!("wait({w})").into_boxed_str()),
            ArgVal::TypeError => kind1 = "type_error",
            ArgVal::NoFact => kind1 = "no_fact",
        }
        trace!(left = kind0, right = kind1, "Lt: classified");

        // Type errors or missing facts on bound AKs fail this op path
        match (&a0, &a1) {
            (ArgVal::TypeError, _) | (_, ArgVal::TypeError) => {
                return PropagatorResult::Contradiction
            }
            (ArgVal::NoFact, _) | (_, ArgVal::NoFact) => return PropagatorResult::Contradiction,
            _ => {}
        }

        // Wait handling
        let mut waits: Vec<usize> = vec![];
        if let ArgVal::Wait(w) = a0 {
            if !store.bindings.contains_key(&w) {
                waits.push(w);
            }
        }
        if let ArgVal::Wait(w) = a1 {
            if !store.bindings.contains_key(&w) {
                waits.push(w);
            }
        }
        if !waits.is_empty() {
            trace!(?waits, "Lt: suspending");
            waits.sort();
            waits.dedup();
            return PropagatorResult::Suspend { on: waits };
        }

        // Both should be ground now
        let (i0, prem0) = match a0 {
            ArgVal::Ground { i, premises } => (i, premises),
            _ => unreachable!(),
        };
        let (i1, prem1) = match a1 {
            ArgVal::Ground { i, premises } => (i, premises),
            _ => unreachable!(),
        };

        if i0 < i1 {
            let mut premises = Vec::new();
            premises.extend(prem0);
            premises.extend(prem1);
            if premises.is_empty() {
                trace!(i0, i1, "Lt: entailed from literals");
                PropagatorResult::Entailed {
                    bindings: vec![],
                    op_tag: OpTag::FromLiterals,
                }
            } else {
                trace!(i0, i1, prem = premises.len(), "Lt: entailed with premises");
                PropagatorResult::Entailed {
                    bindings: vec![],
                    op_tag: OpTag::Derived { premises },
                }
            }
        } else {
            trace!(i0, i1, "Lt: contradiction (not less)");
            PropagatorResult::Contradiction
        }
    }
}

/// Structural copy of Lt matching template shape; can bind wildcard value when AK root bound.
pub struct CopyLtHandler;

impl OpHandler for CopyLtHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }
        // no direct Statement matching in copy path; use binary_view
        let left = &args[0];
        let right = &args[1];
        let mut choices: Vec<crate::prop::Choice> = Vec::new();
        match (left, right) {
            // Value wildcards: copy from Lt(lit, lit) facts
            (StatementTmplArg::Wildcard(wl), StatementTmplArg::Wildcard(wr)) => {
                // If both unbound, enumerate all lit-lit Lt facts and bind both via normalized view
                if !store.bindings.contains_key(&wl.index)
                    && !store.bindings.contains_key(&wr.index)
                {
                    for row in binary_view(edb, BinaryPred::Lt, ArgSel::Val, ArgSel::Val) {
                        if let (Some(l), Some(r)) = (row.left.as_literal(), row.right.as_literal())
                        {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wl.index, l.clone()), (wr.index, r.clone())],
                                op_tag: crate::types::OpTag::CopyStatement { source: row.src },
                            });
                        }
                    }
                }
                // If left is bound, bind right from any; if right is bound, bind left from any
                if let Some(vl) = store.bindings.get(&wl.index) {
                    for row in binary_view(edb, BinaryPred::Lt, ArgSel::Literal(vl), ArgSel::Val) {
                        if let Some(r) = row.right.as_literal() {
                            if !store.bindings.contains_key(&wr.index) {
                                choices.push(crate::prop::Choice {
                                    bindings: vec![(wr.index, r.clone())],
                                    op_tag: crate::types::OpTag::CopyStatement { source: row.src },
                                });
                            }
                        }
                    }
                }
                if let Some(vr) = store.bindings.get(&wr.index) {
                    for row in binary_view(edb, BinaryPred::Lt, ArgSel::Val, ArgSel::Literal(vr)) {
                        if let Some(l) = row.left.as_literal() {
                            if !store.bindings.contains_key(&wl.index) {
                                choices.push(crate::prop::Choice {
                                    bindings: vec![(wl.index, l.clone())],
                                    op_tag: crate::types::OpTag::CopyStatement { source: row.src },
                                });
                            }
                        }
                    }
                }
            }
            // V–? and ?–V: bind the other from copied rows
            (StatementTmplArg::Literal(vl), StatementTmplArg::Wildcard(wr)) => {
                for row in binary_view(edb, BinaryPred::Lt, ArgSel::Literal(vl), ArgSel::Val) {
                    if let Some(vr) = row.right.as_literal() {
                        choices.push(crate::prop::Choice {
                            bindings: vec![(wr.index, vr.clone())],
                            op_tag: crate::types::OpTag::CopyStatement { source: row.src },
                        });
                    }
                }
            }
            (StatementTmplArg::Wildcard(wl), StatementTmplArg::Literal(vr)) => {
                for row in binary_view(edb, BinaryPred::Lt, ArgSel::Val, ArgSel::Literal(vr)) {
                    if let Some(vl) = row.left.as_literal() {
                        choices.push(crate::prop::Choice {
                            bindings: vec![(wl.index, vl.clone())],
                            op_tag: crate::types::OpTag::CopyStatement { source: row.src },
                        });
                    }
                }
            }
            (StatementTmplArg::AnchoredKey(wc_l, key_l), StatementTmplArg::Literal(val_r)) => {
                for v in binary_view(
                    edb,
                    BinaryPred::Lt,
                    ArgSel::AkByKey(key_l),
                    ArgSel::Literal(val_r),
                ) {
                    if let Some((root, _)) = v.left.as_ak() {
                        choices.push(crate::prop::Choice {
                            bindings: vec![(wc_l.index, Value::from(*root))],
                            op_tag: crate::types::OpTag::CopyStatement { source: v.src },
                        });
                    }
                }
                // Wildcard bound on the right to a value → treat as literal
            }
            (StatementTmplArg::AnchoredKey(wc_l, key_l), StatementTmplArg::Wildcard(wv)) => {
                if let Some(v) = store.bindings.get(&wv.index) {
                    for view in binary_view(
                        edb,
                        BinaryPred::Lt,
                        ArgSel::AkByKey(key_l),
                        ArgSel::Literal(v),
                    ) {
                        if let Some((root, _)) = view.left.as_ak() {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wc_l.index, Value::from(*root))],
                                op_tag: crate::types::OpTag::CopyStatement { source: view.src },
                            });
                        }
                    }
                }
                // Root bound and wildcard unbound → bind wildcard from copy
                if let Some(root) = crate::util::bound_root(store, wc_l.index) {
                    for row in binary_view(
                        edb,
                        BinaryPred::Lt,
                        ArgSel::AkExact {
                            root: &root,
                            key: key_l,
                        },
                        ArgSel::Val,
                    ) {
                        if let Some(val) = row.right.as_literal() {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wv.index, val.clone())],
                                op_tag: crate::types::OpTag::CopyStatement { source: row.src },
                            });
                        }
                    }
                }
            }
            (StatementTmplArg::Literal(val_l), StatementTmplArg::AnchoredKey(wc_r, key_r)) => {
                for v in binary_view(
                    edb,
                    BinaryPred::Lt,
                    ArgSel::Literal(val_l),
                    ArgSel::AkByKey(key_r),
                ) {
                    if let Some((root, _)) = v.right.as_ak() {
                        choices.push(crate::prop::Choice {
                            bindings: vec![(wc_r.index, Value::from(*root))],
                            op_tag: crate::types::OpTag::CopyStatement { source: v.src },
                        });
                    }
                }
            }
            (StatementTmplArg::Wildcard(wv), StatementTmplArg::AnchoredKey(wc_r, key_r)) => {
                if let Some(v) = store.bindings.get(&wv.index) {
                    for view in binary_view(
                        edb,
                        BinaryPred::Lt,
                        ArgSel::Literal(v),
                        ArgSel::AkByKey(key_r),
                    ) {
                        if let Some((root, _)) = view.right.as_ak() {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wc_r.index, Value::from(*root))],
                                op_tag: crate::types::OpTag::CopyStatement { source: view.src },
                            });
                        }
                    }
                }
                if let Some(root) = crate::util::bound_root(store, wc_r.index) {
                    for row in binary_view(
                        edb,
                        BinaryPred::Lt,
                        ArgSel::Val,
                        ArgSel::AkExact {
                            root: &root,
                            key: key_r,
                        },
                    ) {
                        if let Some(val) = row.left.as_literal() {
                            choices.push(crate::prop::Choice {
                                bindings: vec![(wv.index, val.clone())],
                                op_tag: crate::types::OpTag::CopyStatement { source: row.src },
                            });
                        }
                    }
                }
            }
            (
                StatementTmplArg::AnchoredKey(wc_l, key_l),
                StatementTmplArg::AnchoredKey(wc_r, key_r),
            ) => {
                for v in binary_view(
                    edb,
                    BinaryPred::Lt,
                    ArgSel::AkByKey(key_l),
                    ArgSel::AkByKey(key_r),
                ) {
                    if let (Some((rl, _)), Some((rr, _))) = (v.left.as_ak(), v.right.as_ak()) {
                        choices.push(crate::prop::Choice {
                            bindings: vec![
                                (wc_l.index, Value::from(*rl)),
                                (wc_r.index, Value::from(*rr)),
                            ],
                            op_tag: crate::types::OpTag::CopyStatement {
                                source: v.src.clone(),
                            },
                        });
                    }
                }
            }
            _ => {}
        }
        if choices.is_empty() {
            // Suspend on referenced unbound wildcards
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

pub fn register_lt_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(NativePredicate::Lt, Box::new(LtFromEntriesHandler));
    reg.register(NativePredicate::Lt, Box::new(CopyLtHandler));
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{containers::Dictionary, Params, StatementTmplArg};

    use super::*;
    use crate::{
        edb::MockEdbView,
        test_helpers::{self, args_from},
        types::{ConstraintStore, PodRef},
        OpTag,
    };

    #[test]
    fn lt_from_entries_literals() {
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = LtFromEntriesHandler;
        let args = args_from("REQUEST(Lt(3, 5))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => {
                assert!(matches!(op_tag, OpTag::FromLiterals));
            }
            other => panic!("unexpected result: {other:?}"),
        }
        let args_false = args_from("REQUEST(Lt(5, 3))");
        let res2 = handler.propagate(&args_false, &mut store, &edb);
        assert!(matches!(res2, PropagatorResult::Contradiction));
    }

    #[test]
    fn lt_from_entries_ak_lit_generated() {
        // Lt(?R["k"], 10) with bound root and full dict k:7
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
        let handler = LtFromEntriesHandler;
        let args = args_from("REQUEST(Lt(?R[\"k\"], 10))");
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
    fn lt_from_entries_ak_ak_both_bound() {
        // Lt(?L["a"], ?R["b"]) with both bound and 3 < 5
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
        let handler = LtFromEntriesHandler;
        let args = args_from(r#"REQUEST(Lt(?L["a"], ?R["b"]))"#);
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
    fn lt_from_entries_suspend_unbound() {
        // Lt(?L["a"], 10) with unbound left root should suspend
        let edb = MockEdbView::default();
        let mut store = ConstraintStore::default();
        let handler = LtFromEntriesHandler;
        let args = args_from("REQUEST(Lt(?L[\"a\"], 10))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Suspend { on } => assert!(on.contains(&0)),
            other => panic!("expected Suspend, got {other:?}"),
        }
    }

    #[test]
    fn copy_lt_binds_value_from_left_ak_when_root_bound() {
        // Given Lt(R["k"], 10) in EDB, CopyLt should bind ?X when ?R bound
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(7))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = PodRef(r);
        edb.add_lt_row_lak_rval(r, test_helpers::key("k"), Value::from(10), src.clone());

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(r)); // ?R
        let handler = CopyLtHandler;
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
    fn copy_lt_binds_root_from_right_ak_when_value_bound() {
        // Given Lt(10, R["k"]) in EDB, CopyLt should bind ?R when ?X bound
        let mut edb = MockEdbView::default();
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(20))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = PodRef(r);
        edb.add_lt_row_lval_rak(Value::from(10), r, test_helpers::key("k"), src.clone());

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(10)); // ?X left
        let handler = CopyLtHandler;
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
    fn copy_lt_binds_both_wildcards_from_vv_fact() {
        // Lt(?X, ?Y) should bind both from Lt(3, 5) fact
        let mut edb = MockEdbView::default();
        let src = PodRef(test_helpers::root("s"));
        edb.add_lt_row_vals(Value::from(3), Value::from(5), src.clone());

        let mut store = ConstraintStore::default();
        let handler = CopyLtHandler;
        let args = args_from("REQUEST(Lt(?X, ?Y))");
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
    fn copy_lt_binds_one_wildcard_from_vv_partial() {
        // Lt(?X, 5) binds ?X from Lt(3,5); Lt(3, ?Y) binds ?Y from Lt(3,5)
        let mut edb = MockEdbView::default();
        let src = PodRef(test_helpers::root("s"));
        edb.add_lt_row_vals(Value::from(3), Value::from(5), src.clone());

        let mut store = ConstraintStore::default();
        let handler = CopyLtHandler;
        let args1 = args_from("REQUEST(Lt(?X, 5))");
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

        let args2 = args_from("REQUEST(Lt(3, ?Y))");
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
