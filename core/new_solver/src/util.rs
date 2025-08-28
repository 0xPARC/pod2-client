use pod2::middleware::{Hash, Key, Statement, Value, ValueRef};

use crate::{
    edb::{ContainsSource, EdbView},
    prop::Choice,
    types::{ConstraintStore, OpTag},
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
