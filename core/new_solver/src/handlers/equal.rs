use pod2::middleware::{NativePredicate, StatementTmplArg};
use tracing::trace;

use super::util::{arg_to_selector, handle_copy_results};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::{Choice, PropagatorResult},
    types::{ConstraintStore, OpTag},
    util::{
        bound_root, contains_stmt, entailed_if_both_bound_equal, entailed_if_bound_matches,
        enumerate_choices_for, enumerate_other_root_choices, tag_from_source,
    },
};

/// Structural copy of Equal matching the template shape (AK–V, V–AK, AK–AK, V–V).
pub struct CopyEqualHandler;

impl OpHandler for CopyEqualHandler {
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
            crate::edb::PredicateKey::Native(NativePredicate::Equal),
            &[lhs, rhs],
        );

        handle_copy_results(results, args, store)
    }
}

/// Value-centric EqualFromEntries: use Contains or roots_with_key_value to bind roots.
pub struct EqualFromEntriesHandler;

impl OpHandler for EqualFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 2 {
            return PropagatorResult::Contradiction;
        }
        trace!("EqualFromEntries: start");
        let left = &args[0];
        let right = &args[1];
        let mut choices: Vec<Choice> = Vec::new();
        match (left, right) {
            // Pure value equality from literals/bound wildcards
            (StatementTmplArg::Literal(vl), StatementTmplArg::Literal(vr)) => {
                if vl == vr {
                    return PropagatorResult::Entailed {
                        bindings: vec![],
                        op_tag: OpTag::FromLiterals,
                    };
                } else {
                    return PropagatorResult::Contradiction;
                }
            }
            // Both wildcards: if both bound, check; if one bound, bind the other
            (StatementTmplArg::Wildcard(wl), StatementTmplArg::Wildcard(wr)) => {
                let lb = store.bindings.get(&wl.index).cloned();
                let rb = store.bindings.get(&wr.index).cloned();
                match (lb, rb) {
                    (Some(lv), Some(rv)) => {
                        if lv == rv {
                            return PropagatorResult::Entailed {
                                bindings: vec![],
                                op_tag: OpTag::FromLiterals,
                            };
                        } else {
                            return PropagatorResult::Contradiction;
                        }
                    }
                    (Some(lv), None) => {
                        return PropagatorResult::Entailed {
                            bindings: vec![(wr.index, lv)],
                            op_tag: OpTag::FromLiterals,
                        };
                    }
                    (None, Some(rv)) => {
                        return PropagatorResult::Entailed {
                            bindings: vec![(wl.index, rv)],
                            op_tag: OpTag::FromLiterals,
                        };
                    }
                    (None, None) => {}
                }
            }
            (StatementTmplArg::Wildcard(wv), StatementTmplArg::Literal(vr)) => {
                if let Some(bv) = store.bindings.get(&wv.index) {
                    if bv == vr {
                        return PropagatorResult::Entailed {
                            bindings: vec![],
                            op_tag: OpTag::FromLiterals,
                        };
                    } else {
                        return PropagatorResult::Contradiction;
                    }
                } else {
                    // Bind unbound wildcard to the literal
                    return PropagatorResult::Entailed {
                        bindings: vec![(wv.index, vr.clone())],
                        op_tag: OpTag::FromLiterals,
                    };
                }
            }
            (StatementTmplArg::Literal(vl), StatementTmplArg::Wildcard(wv)) => {
                if let Some(bv) = store.bindings.get(&wv.index) {
                    if bv == vl {
                        return PropagatorResult::Entailed {
                            bindings: vec![],
                            op_tag: OpTag::FromLiterals,
                        };
                    } else {
                        return PropagatorResult::Contradiction;
                    }
                } else {
                    // Bind unbound wildcard to the literal
                    return PropagatorResult::Entailed {
                        bindings: vec![(wv.index, vl.clone())],
                        op_tag: OpTag::FromLiterals,
                    };
                }
            }
            // AK–Wildcard(value)
            (StatementTmplArg::AnchoredKey(wc_l, key_l), StatementTmplArg::Wildcard(wv)) => {
                if let Some(bound_val) = store.bindings.get(&wv.index) {
                    if let Some(root) = bound_root(store, wc_l.index) {
                        if let Some(ent) = entailed_if_bound_matches(root, key_l, bound_val, edb) {
                            return ent;
                        }
                    } else {
                        choices.extend(enumerate_choices_for(key_l, bound_val, wc_l.index, edb));
                    }
                } else if let Some(root) = bound_root(store, wc_l.index) {
                    if let Some(val) = edb.contains_value(&root, key_l) {
                        if let Some(src) = edb.contains_source(&root, key_l, &val) {
                            let tag = tag_from_source(key_l, &val, src);
                            let c = contains_stmt(root, key_l, val.clone());
                            return PropagatorResult::Entailed {
                                bindings: vec![(wv.index, val)],
                                op_tag: OpTag::Derived {
                                    premises: vec![(c, tag)],
                                },
                            };
                        }
                    } else {
                        return PropagatorResult::Contradiction;
                    }
                }
            }
            // Wildcard(value)–AK
            (StatementTmplArg::Wildcard(wv), StatementTmplArg::AnchoredKey(wc_r, key_r)) => {
                if let Some(bound_val) = store.bindings.get(&wv.index) {
                    if let Some(root) = bound_root(store, wc_r.index) {
                        if let Some(ent) = entailed_if_bound_matches(root, key_r, bound_val, edb) {
                            return ent;
                        }
                    } else {
                        choices.extend(enumerate_choices_for(key_r, bound_val, wc_r.index, edb));
                    }
                } else if let Some(root) = bound_root(store, wc_r.index) {
                    if let Some(val) = edb.contains_value(&root, key_r) {
                        if let Some(src) = edb.contains_source(&root, key_r, &val) {
                            let tag = tag_from_source(key_r, &val, src);
                            let c = contains_stmt(root, key_r, val.clone());
                            return PropagatorResult::Entailed {
                                bindings: vec![(wv.index, val)],
                                op_tag: OpTag::Derived {
                                    premises: vec![(c, tag)],
                                },
                            };
                        }
                    } else {
                        return PropagatorResult::Contradiction;
                    }
                }
            }
            // AK–V
            (StatementTmplArg::AnchoredKey(wc_l, key_l), StatementTmplArg::Literal(val_r)) => {
                if let Some(root) = bound_root(store, wc_l.index) {
                    if let Some(ent) = entailed_if_bound_matches(root, key_l, val_r, edb) {
                        return ent;
                    }
                } else {
                    choices.extend(enumerate_choices_for(key_l, val_r, wc_l.index, edb));
                }
            }
            // V–AK
            (StatementTmplArg::Literal(val_l), StatementTmplArg::AnchoredKey(wc_r, key_r)) => {
                if let Some(root) = bound_root(store, wc_r.index) {
                    if let Some(ent) = entailed_if_bound_matches(root, key_r, val_l, edb) {
                        return ent;
                    }
                } else {
                    choices.extend(enumerate_choices_for(key_r, val_l, wc_r.index, edb));
                }
            }
            // AK–AK
            (
                StatementTmplArg::AnchoredKey(wc_l, key_l),
                StatementTmplArg::AnchoredKey(wc_r, key_r),
            ) => {
                let lroot = bound_root(store, wc_l.index);
                let rroot = bound_root(store, wc_r.index);
                match (lroot, rroot) {
                    (Some(rl), Some(rr)) => {
                        if let Some(ent) = entailed_if_both_bound_equal(rl, key_l, rr, key_r, edb) {
                            return ent;
                        }
                    }
                    (Some(rl), None) => {
                        if let Some(vl) = edb.contains_value(&rl, key_l) {
                            choices
                                .extend(enumerate_other_root_choices(&vl, key_r, wc_r.index, edb));
                        }
                    }
                    (None, Some(rr)) => {
                        if let Some(vr) = edb.contains_value(&rr, key_r) {
                            choices
                                .extend(enumerate_other_root_choices(&vr, key_l, wc_l.index, edb));
                        }
                    }
                    (None, None) => {
                        // Both unbound -> no choices (no guessing); future: suspend on both
                    }
                }
            }
            _ => {}
        }
        if choices.is_empty() {
            // Under-constrained: suspend on any referenced wildcards
            let waits_all = crate::prop::wildcards_in_args(args);
            let waits: Vec<_> = waits_all
                .into_iter()
                .filter(|i| !store.bindings.contains_key(i))
                .collect();
            if waits.is_empty() {
                trace!("EqualFromEntries: contradiction (no choices, no waits)");
                PropagatorResult::Contradiction
            } else {
                trace!(?waits, "EqualFromEntries: suspend on");
                PropagatorResult::Suspend { on: waits }
            }
        } else {
            trace!(alts = choices.len(), "EqualFromEntries: choices produced");
            PropagatorResult::Choices {
                alternatives: choices,
            }
        }
    }
}

pub fn register_equal_handlers(reg: &mut crate::op::OpRegistry) {
    reg.register(NativePredicate::Equal, Box::new(CopyEqualHandler));
    reg.register(NativePredicate::Equal, Box::new(EqualFromEntriesHandler));
}

#[cfg(test)]
mod tests {
    use pod2::middleware::{containers::Dictionary, AnchoredKey, Params, Statement, Value};

    use super::*;
    use crate::{
        edb::ImmutableEdbBuilder,
        test_helpers::{self, args_from},
        types::{ConstraintStore, PodRef},
    };

    #[test]
    fn equal_from_entries_ak_v_generated_bound() {
        // Equal(?R["k"], 1) with bound R and full dict containing (k -> 1)
        // Build a real dictionary with {k:1}
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        let mut store = ConstraintStore::default();
        // wildcard index is 0 for first ?R variable in a simple REQUEST
        store.bindings.insert(0, Value::from(r));

        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?R[\"k\"], 1))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => match op_tag {
                OpTag::Derived { premises } => {
                    assert_eq!(premises.len(), 1);
                    match &premises[0].1 {
                        OpTag::GeneratedContains { root, .. } => assert_eq!(*root, r),
                        other => panic!("unexpected tag: {other:?}"),
                    }
                }
                other => panic!("unexpected tag: {other:?}"),
            },
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_ak_v_bound_to_full_dictionary_value_normalizes_root() {
        // Bind the AK root wildcard to a full Dictionary value; handler should normalize to its commitment
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let root = dict.commitment();
        let edb = ImmutableEdbBuilder::new()
            .add_full_dict(dict.clone())
            .build();

        let mut store = ConstraintStore::default();
        // Attempt to bind wildcard to the full dictionary value (normalize to commitment internally)
        store.bindings.insert(0, Value::from(dict));

        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?R[\"k\"], 1))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => match op_tag {
                OpTag::Derived { premises } => {
                    assert_eq!(premises.len(), 1);
                    match &premises[0].1 {
                        OpTag::GeneratedContains { root: r, .. } => assert_eq!(*r, root),
                        other => panic!("unexpected tag: {other:?}"),
                    }
                }
                other => panic!("unexpected tag: {other:?}"),
            },
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn copy_equal_binds_wildcard_from_left_ak() {
        // CopyEqual should bind ?X when ?R is bound and Equal(R["k"], 1) exists to copy
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = PodRef(r);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::Equal(AnchoredKey::new(r, test_helpers::key("k")).into(), 1.into()),
                src.clone(),
            )
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(r)); // bind ?R

        let handler = CopyEqualHandler;
        let args = args_from(r#"REQUEST(Equal(?R["k"], ?X))"#);
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                let ch = &alternatives[0];
                assert_eq!(ch.bindings[0].0, 1); // ?X index
                assert_eq!(ch.bindings[0].1, Value::from(1));
                match &ch.op_tag {
                    OpTag::CopyStatement { source } => assert_eq!(*source, src),
                    other => panic!("unexpected tag: {other:?}"),
                }
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn copy_equal_binds_wildcard_from_right_ak() {
        // CopyEqual should bind ?X when ?R is bound and Equal(1, R["k"]) exists to copy
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let src = PodRef(r);
        // Add Equal(lit, AK)
        let st = Statement::Equal(1.into(), AnchoredKey::new(r, test_helpers::key("k")).into());
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(st, src.clone())
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(1, Value::from(r)); // bind ?R (second wildcard)

        let handler = CopyEqualHandler;
        let args = args_from(r#"REQUEST(Equal(?X, ?R["k"]))"#);
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                let ch = &alternatives[0];
                assert_eq!(ch.bindings[0].0, 0); // ?X index
                assert_eq!(ch.bindings[0].1, Value::from(1));
                match &ch.op_tag {
                    OpTag::CopyStatement { source } => assert_eq!(*source, src),
                    other => panic!("unexpected tag: {other:?}"),
                }
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_ak_v_copied_unbound() {
        // Equal(?R["k"], 1) with unbound R and only a copied Contains fact
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let pod = PodRef(r);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::Contains(r.into(), "k".into(), 1.into()),
                pod.clone(),
            )
            .build();

        let mut store = ConstraintStore::default();
        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?R[\"k\"], 1))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                let ch = &alternatives[0];
                // binding for wc index 0
                assert_eq!(ch.bindings[0].0, 0);
                // premise should be CopyStatement
                match &ch.op_tag {
                    OpTag::Derived { premises } => match &premises[0].1 {
                        OpTag::CopyStatement { source } => assert_eq!(*source, pod),
                        other => panic!("unexpected tag: {other:?}"),
                    },
                    other => panic!("unexpected tag: {other:?}"),
                }
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_v_ak_generated_unbound() {
        // Equal(1, ?R["k"]) unbound R, full dict
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();
        let mut store = ConstraintStore::default();
        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(1, ?R[\"k\"]))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                match &alternatives[0].op_tag {
                    OpTag::Derived { premises } => match &premises[0].1 {
                        OpTag::GeneratedContains { root, .. } => assert_eq!(*root, r),
                        other => panic!("unexpected tag: {other:?}"),
                    },
                    other => panic!("unexpected tag: {other:?}"),
                }
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_v_ak_copied_unbound() {
        // Equal(1, ?R["k"]) unbound R, only copied Contains
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let pod = PodRef(r);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::Contains(r.into(), "k".into(), 1.into()),
                pod.clone(),
            )
            .build();

        let mut store = ConstraintStore::default();
        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(1, ?R[\"k\"]))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                match &alternatives[0].op_tag {
                    OpTag::Derived { premises } => match &premises[0].1 {
                        OpTag::CopyStatement { source } => assert_eq!(*source, pod),
                        other => panic!("unexpected tag: {other:?}"),
                    },
                    other => panic!("unexpected tag: {other:?}"),
                }
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_ak_ak_one_bound_enumerate_generated() {
        // Equal(?L["a"], ?R["b"]) with left bound (value 7), right unbound; enumerate right from full dict
        let params = Params::default();
        // Left dict has a:7 (can be copied or full; contains_value prefers copied if both)
        let dict_l = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("a"), Value::from(7))].into(),
        )
        .unwrap();
        let rl = dict_l.commitment();
        // Register copied fact for left so contains_value works
        let podl = PodRef(rl);
        // Right dict has b:7 as full dict to generate
        let dict_r = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("b"), Value::from(7))].into(),
        )
        .unwrap();
        let rr = dict_r.commitment();
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::Contains(rl.into(), "a".into(), 7.into()), podl)
            .add_full_dict(dict_r)
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(rl)); // bind left

        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?L[\"a\"], ?R[\"b\"]))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                // Should enumerate the right root rr
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 1 && v.raw() == Value::from(rr).raw())));
                // Check premise tag is GeneratedContains for the enumerated side
                for ch in alternatives.iter() {
                    if ch
                        .bindings
                        .iter()
                        .any(|(i, v)| *i == 1 && v.raw() == Value::from(rr).raw())
                    {
                        match &ch.op_tag {
                            OpTag::Derived { premises } => match &premises[0].1 {
                                OpTag::GeneratedContains { root, .. } => assert_eq!(*root, rr),
                                other => panic!("unexpected tag: {other:?}"),
                            },
                            other => panic!("unexpected tag: {other:?}"),
                        }
                    }
                }
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_ak_ak_one_bound_enumerate_copied() {
        // Equal(?L["a"], ?R["b"]) with left bound (value 7), right unbound; enumerate right from copied Contains
        let params = Params::default();
        // Left dict has a:7
        let dict_l = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("a"), Value::from(7))].into(),
        )
        .unwrap();
        let rl = dict_l.commitment();
        let podl = PodRef(rl);
        // Right copied contains b:7
        let dict_r = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("b"), Value::from(7))].into(),
        )
        .unwrap();
        let rr = dict_r.commitment();
        let podr = PodRef(rr);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::Contains(rl.into(), "a".into(), 7.into()),
                podl.clone(),
            )
            .add_statement_for_test(
                Statement::Contains(rr.into(), "b".into(), 7.into()),
                podr.clone(),
            )
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(rl)); // bind left

        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?L[\"a\"], ?R[\"b\"]))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                // Should enumerate the right root rr
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 1 && v.raw() == Value::from(rr).raw())));
                // Check premise tag is CopyStatement for the enumerated side
                for ch in alternatives.iter() {
                    if ch
                        .bindings
                        .iter()
                        .any(|(i, v)| *i == 1 && v.raw() == Value::from(rr).raw())
                    {
                        match &ch.op_tag {
                            OpTag::Derived { premises } => match &premises[0].1 {
                                OpTag::CopyStatement { source } => assert_eq!(*source, podr),
                                other => panic!("unexpected tag: {other:?}"),
                            },
                            other => panic!("unexpected tag: {other:?}"),
                        }
                    }
                }
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_ak_ak_both_unbound_suspends() {
        // Both AK roots unbound; handler should suspend on both wildcard indices
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = EqualFromEntriesHandler;
        let args = args_from(r#"REQUEST(Equal(?L["a"], ?R["b"]))"#);
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Suspend { on } => {
                // Expect both wildcard indices 0 and 1
                assert!(on.contains(&0) && on.contains(&1));
            }
            other => panic!("expected Suspend, got {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_negative_no_match() {
        // AK–V: query Equal(?R["k"], 1) but only have k:2 in full dict → no choices
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(2))].into(),
        )
        .unwrap();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        let mut store = ConstraintStore::default();
        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?R[\"k\"], 1))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            // Under-constrained: unbound root, no matches → Suspend on root wildcard
            PropagatorResult::Suspend { on } => assert!(on.contains(&0)),
            other => panic!("expected Suspend, got {other:?}"),
        }

        // AK–AK both bound with different values → no entailment
        let dict_l = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("a"), Value::from(3))].into(),
        )
        .unwrap();
        let dict_r = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("b"), Value::from(4))].into(),
        )
        .unwrap();
        let rl = dict_l.commitment();
        let rr = dict_r.commitment();
        let edb2 = ImmutableEdbBuilder::new()
            .add_full_dict(dict_l)
            .add_full_dict(dict_r)
            .build();

        let mut store2 = ConstraintStore::default();
        store2.bindings.insert(0, Value::from(rl));
        store2.bindings.insert(1, Value::from(rr));
        let args2 = args_from("REQUEST(Equal(?L[\"a\"], ?R[\"b\"]))");
        let res2 = handler.propagate(&args2, &mut store2, &edb2);
        match res2 {
            PropagatorResult::Contradiction => {}
            other => panic!("expected Contradiction, got {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_ak_ak_both_bound_equal_mixed_sources() {
        // Equal(?L["a"], ?R["b"]) with both roots bound, values equal; left copied, right generated
        let params = Params::default();
        let dict_l = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("a"), Value::from(7))].into(),
        )
        .unwrap();
        let dict_r = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("b"), Value::from(7))].into(),
        )
        .unwrap();
        let rl = dict_l.commitment();
        let rr = dict_r.commitment();
        let podl = PodRef(rl);
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(Statement::Contains(rl.into(), "a".into(), 7.into()), podl)
            .add_full_dict(dict_r)
            .build();

        let mut store = ConstraintStore::default();
        store.bindings.insert(0, Value::from(rl));
        store.bindings.insert(1, Value::from(rr));

        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?L[\"a\"], ?R[\"b\"]))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { op_tag, .. } => match op_tag {
                OpTag::Derived { premises } => {
                    assert_eq!(premises.len(), 2);
                    // Order agnostic check
                    let tags: Vec<&OpTag> = premises.iter().map(|p| &p.1).collect();
                    assert!(tags
                        .iter()
                        .any(|t| matches!(t, OpTag::CopyStatement { .. })));
                    assert!(tags
                        .iter()
                        .any(|t| matches!(t, OpTag::GeneratedContains { .. })));
                }
                other => panic!("unexpected tag: {other:?}"),
            },
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_ak_wildcard_bind_value_when_root_bound() {
        // Equal(?R["k"], ?X) with bound R and full dict containing (k -> 1) binds ?X=1
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();

        let mut store = ConstraintStore::default();
        // wildcard index 0 for ?R, 1 for ?X in this template order
        store.bindings.insert(0, Value::from(r));

        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?R[\"k\"], ?X))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Entailed { bindings, op_tag } => {
                assert_eq!(bindings.len(), 1);
                assert_eq!(bindings[0].0, 1);
                assert_eq!(bindings[0].1, Value::from(1));
                match op_tag {
                    OpTag::Derived { premises } => {
                        assert_eq!(premises.len(), 1);
                        assert!(matches!(premises[0].1, OpTag::GeneratedContains { .. }));
                    }
                    other => panic!("unexpected tag: {other:?}"),
                }
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_ak_wildcard_unbound_suspends() {
        // Equal(?R["k"], ?X) with both unbound should suspend (no guessing)
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?R[\"k\"], ?X))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Suspend { on } => {
                assert!(on.contains(&0));
                assert!(on.contains(&1));
            }
            other => panic!("expected Suspend, got {other:?}"),
        }
    }

    #[test]
    fn equal_from_entries_wildcard_ak_bound_value_enumerates() {
        // Equal(?X, ?R["k"]) with ?X bound to 1 enumerates roots with k->1
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            [(test_helpers::key("k"), Value::from(1))].into(),
        )
        .unwrap();
        let r = dict.commitment();
        let edb = ImmutableEdbBuilder::new().add_full_dict(dict).build();
        let mut store = ConstraintStore::default();
        // ?X is first wildcard (index 0), ?R is index 1
        store.bindings.insert(0, Value::from(1));
        let handler = EqualFromEntriesHandler;
        let args = args_from("REQUEST(Equal(?X, ?R[\"k\"]))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives.iter().any(|ch| ch
                    .bindings
                    .iter()
                    .any(|(i, v)| *i == 1 && v.raw() == Value::from(r).raw())));
            }
            other => panic!("expected Choices, got {other:?}"),
        }
    }
}
