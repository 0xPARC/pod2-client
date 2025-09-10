use std::collections::HashMap;

use hex::ToHex;
use pod2::middleware::{
    AnchoredKey, Hash, Key, NativePredicate, Predicate, Statement, StatementTmpl, StatementTmplArg,
    Value, ValueRef,
};

use crate::{
    edb::{ContainsSource, EdbView},
    prop::Choice,
    types::{ConstraintStore, OpTag, PodRef},
};

/// If the wildcard at `wc_index` is bound to a root-like value, return its commitment hash.
pub fn bound_root(store: &ConstraintStore, wc_index: usize) -> Option<Hash> {
    store.bindings.get(&wc_index).map(|v| Hash::from(v.raw()))
}

/// Helper to build a Contains(root, key, value) statement from primitives.
pub fn contains_stmt(root: Hash, key: &Key, value: Value) -> Statement {
    Statement::Contains(
        ValueRef::Literal(Value::from(root)),
        ValueRef::Literal(Value::from(key.name())),
        ValueRef::Literal(value),
    )
}

/// Map a ContainsSource into an OpTag, attaching key/value for GeneratedContains.
pub fn tag_from_source(key: &Key, value: &Value, src: ContainsSource) -> OpTag {
    match src {
        ContainsSource::Copied { pod } => OpTag::CopyStatement { source: pod },
        ContainsSource::GeneratedFromFullDict { root } => OpTag::GeneratedContains {
            root,
            key: key.clone(),
            value: value.clone(),
        },
    }
}

/// Enumerate choices binding a wildcard root for a (key, value) pair using EDB provenance.
pub fn enumerate_choices_for(
    key: &Key,
    value: &Value,
    wc_index: usize,
    edb: &dyn EdbView,
) -> Vec<Choice> {
    let mut out = Vec::new();
    for (root, src) in edb.enumerate_contains_sources(key, value) {
        let tag = tag_from_source(key, value, src);
        let c = contains_stmt(root, key, value.clone());
        out.push(Choice {
            bindings: vec![(wc_index, Value::from(root))],
            op_tag: OpTag::Derived {
                premises: vec![(c, tag)],
            },
        });
    }
    out
}

/// If a bound root has a Contains(root,key,value), return an Entailed result with one premise.
pub fn entailed_if_bound_matches(
    root: Hash,
    key: &Key,
    value: &Value,
    edb: &dyn EdbView,
) -> Option<crate::prop::PropagatorResult> {
    edb.contains_source(&root, key, value).map(|src| {
        let tag = tag_from_source(key, value, src);
        let c = contains_stmt(root, key, value.clone());
        crate::prop::PropagatorResult::Entailed {
            bindings: vec![],
            op_tag: OpTag::Derived {
                premises: vec![(c, tag)],
            },
        }
    })
}

/// If both bound roots have equal values at keys, entail with two premises; else None.
pub fn entailed_if_both_bound_equal(
    rl: Hash,
    key_l: &Key,
    rr: Hash,
    key_r: &Key,
    edb: &dyn EdbView,
) -> Option<crate::prop::PropagatorResult> {
    let vl = edb.contains_value(&rl, key_l)?;
    let vr = edb.contains_value(&rr, key_r)?;
    if vl != vr {
        return None;
    }
    let tag1 = tag_from_source(key_l, &vl, edb.contains_source(&rl, key_l, &vl)?);
    let tag2 = tag_from_source(key_r, &vr, edb.contains_source(&rr, key_r, &vr)?);
    let c1 = contains_stmt(rl, key_l, vl);
    let c2 = contains_stmt(rr, key_r, vr);
    Some(crate::prop::PropagatorResult::Entailed {
        bindings: vec![],
        op_tag: OpTag::Derived {
            premises: vec![(c1, tag1), (c2, tag2)],
        },
    })
}

/// Given a bound value and the other AK's key, enumerate choices for the other root.
pub fn enumerate_other_root_choices(
    bound_val: &Value,
    other_key: &Key,
    other_wc_index: usize,
    edb: &dyn EdbView,
) -> Vec<Choice> {
    enumerate_choices_for(other_key, bound_val, other_wc_index, edb)
}

/// Compute unique operation and input counts by traversing the proof DAG in `store.premises`.
/// - ops: number of unique concrete `Statement`s across all premises (including nested `Derived`/`CustomDeduction`).
/// - inputs: number of unique `PodRef` sources from `CopyStatement` op-tags (including nested).
pub fn proof_cost(store: &ConstraintStore) -> (usize, usize) {
    use std::collections::{HashSet, VecDeque};
    // Canonicalize a Statement into a stable string key using predicate and arg raw commitments
    fn stmt_key(st: &Statement) -> String {
        let mut s = String::new();
        s.push_str(&format!("{:?}|", st.predicate()));
        for arg in st.args().into_iter() {
            match arg {
                pod2::middleware::StatementArg::Literal(v) => {
                    s.push_str(&v.raw().encode_hex::<String>());
                    s.push('|');
                }
                pod2::middleware::StatementArg::Key(ak) => {
                    s.push_str(&ak.root.encode_hex::<String>());
                    s.push(':');
                    s.push_str(ak.key.name());
                    s.push('|');
                }
                pod2::middleware::StatementArg::None => {
                    s.push_str("none|");
                }
            }
        }
        s
    }

    let mut seen_stmts: std::collections::BTreeSet<String> = Default::default();
    let mut seen_inputs: HashSet<PodRef> = HashSet::new();

    // Worklist over op-tags to traverse nested premises
    let mut q: VecDeque<(Statement, OpTag)> = store.premises.clone().into();
    while let Some((st, tag)) = q.pop_front() {
        seen_stmts.insert(stmt_key(&st));
        match tag {
            OpTag::CopyStatement { source } => {
                seen_inputs.insert(source);
            }
            OpTag::Derived { premises } | OpTag::CustomDeduction { premises, .. } => {
                for p in premises.into_iter() {
                    q.push_back(p);
                }
            }
            OpTag::GeneratedContains { .. } | OpTag::FromLiterals => {}
        }
    }
    (seen_stmts.len(), seen_inputs.len())
}

/// Instantiate a concrete head `Statement` from a goal template under current bindings.
/// Returns None if required wildcards are unbound.
pub fn instantiate_goal(
    tmpl: &StatementTmpl,
    bindings: &HashMap<usize, Value>,
) -> Option<Statement> {
    fn arg_to_vr(arg: &StatementTmplArg, bindings: &HashMap<usize, Value>) -> Option<ValueRef> {
        match arg {
            StatementTmplArg::Literal(v) => Some(ValueRef::Literal(v.clone())),
            StatementTmplArg::Wildcard(w) => {
                bindings.get(&w.index).map(|v| ValueRef::Literal(v.clone()))
            }
            StatementTmplArg::AnchoredKey(w, key) => bindings.get(&w.index).map(|v| {
                let root = Hash::from(v.raw());
                ValueRef::Key(AnchoredKey::new(root, key.clone()))
            }),
            StatementTmplArg::None => None,
        }
    }

    match tmpl.pred {
        // TODO better matching
        Predicate::Native(NativePredicate::Equal) => {
            if tmpl.args.len() != 2 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            Some(Statement::Equal(a0, a1))
        }
        Predicate::Native(NativePredicate::NotEqual) => {
            if tmpl.args.len() != 2 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            Some(Statement::NotEqual(a0, a1))
        }
        Predicate::Native(NativePredicate::Lt) => {
            if tmpl.args.len() != 2 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            Some(Statement::Lt(a0, a1))
        }
        Predicate::Native(NativePredicate::LtEq) => {
            if tmpl.args.len() != 2 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            Some(Statement::LtEq(a0, a1))
        }
        Predicate::Native(NativePredicate::Contains) => {
            if tmpl.args.len() != 3 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            let a2 = arg_to_vr(&tmpl.args[2], bindings)?;
            Some(Statement::Contains(a0, a1, a2))
        }
        Predicate::Native(NativePredicate::NotContains) => {
            if tmpl.args.len() != 2 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            Some(Statement::NotContains(a0, a1))
        }
        Predicate::Native(NativePredicate::SignedBy) => {
            if tmpl.args.len() != 2 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            Some(Statement::SignedBy(a0, a1))
        }
        Predicate::Native(NativePredicate::SumOf) => {
            if tmpl.args.len() != 3 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            let a2 = arg_to_vr(&tmpl.args[2], bindings)?;
            Some(Statement::SumOf(a0, a1, a2))
        }
        Predicate::Native(NativePredicate::ProductOf) => {
            if tmpl.args.len() != 3 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            let a2 = arg_to_vr(&tmpl.args[2], bindings)?;
            Some(Statement::ProductOf(a0, a1, a2))
        }
        Predicate::Native(NativePredicate::HashOf) => {
            if tmpl.args.len() != 3 {
                return None;
            }
            let a0 = arg_to_vr(&tmpl.args[0], bindings)?;
            let a1 = arg_to_vr(&tmpl.args[1], bindings)?;
            let a2 = arg_to_vr(&tmpl.args[2], bindings)?;
            Some(Statement::HashOf(a0, a1, a2))
        }
        _ => None,
    }
}

/// Instantiate a custom statement from head args under current bindings.
pub fn instantiate_custom(
    pred: &pod2::middleware::CustomPredicateRef,
    head_args: &[StatementTmplArg],
    bindings: &HashMap<usize, Value>,
) -> Option<Statement> {
    fn arg_to_value(arg: &StatementTmplArg, bindings: &HashMap<usize, Value>) -> Option<Value> {
        match arg {
            StatementTmplArg::Literal(v) => Some(v.clone()),
            StatementTmplArg::Wildcard(w) => bindings.get(&w.index).cloned(),
            // For MVP, disallow AnchoredKey in custom head; require resolved root value in head
            StatementTmplArg::AnchoredKey(_, _) => None,
            StatementTmplArg::None => None,
        }
    }
    let mut vals: Vec<Value> = Vec::with_capacity(head_args.len());
    for a in head_args.iter() {
        vals.push(arg_to_value(a, bindings)?);
    }
    Some(Statement::Custom(pred.clone(), vals))
}
