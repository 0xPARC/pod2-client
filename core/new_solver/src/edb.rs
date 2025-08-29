use std::collections::{HashMap, HashSet};

use pod2::middleware::{
    containers::Dictionary, AnchoredKey, Hash, Key, Statement, Value, ValueRef,
};

use crate::types::PodRef;

/// Minimal read-only EDB interface for OpHandlers in MVP.
pub trait EdbView: Send + Sync {
    fn match_equal_lhs_ak_rhs_val(&self, _key: &Key, _val: &Value) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }

    fn contains_value(&self, _root: &pod2::middleware::Hash, _key: &Key) -> Option<Value> {
        None
    }

    fn roots_with_key_value(&self, _key: &Key, _val: &Value) -> Vec<pod2::middleware::Hash> {
        Vec::new()
    }

    fn equal_lhs_val_rhs_ak(&self, _val: &Value, _key: &Key) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }
    /// CopyEqual support: find Equal(AK(root,key), value) rows regardless of value.
    fn equal_lhs_ak_rhs_any(&self, _root: &Hash, _key: &Key) -> Vec<(Value, PodRef)> {
        Vec::new()
    }

    fn equal_ak_ak_by_keys(&self, _left_key: &Key, _right_key: &Key) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }

    /// CopyEqual support: find Equal(value, AK(root,key)) rows regardless of value.
    fn equal_lhs_any_rhs_ak(&self, _root: &Hash, _key: &Key) -> Vec<(Value, PodRef)> {
        Vec::new()
    }

    // Lt copy helpers
    fn lt_lhs_ak_rhs_val(&self, _key: &Key, _val: &Value) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }
    fn lt_lhs_val_rhs_ak(&self, _val: &Value, _key: &Key) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }
    fn lt_ak_ak_by_keys(&self, _left_key: &Key, _right_key: &Key) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }
    fn lt_lhs_ak_rhs_any(&self, _root: &Hash, _key: &Key) -> Vec<(Value, PodRef)> {
        Vec::new()
    }
    fn lt_lhs_any_rhs_ak(&self, _root: &Hash, _key: &Key) -> Vec<(Value, PodRef)> {
        Vec::new()
    }
    fn lt_lhs_val_rhs_val(&self, _val_l: &Value, _val_r: &Value) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }
    fn lt_lhs_val_rhs_any(&self, _val_l: &Value) -> Vec<(Value, PodRef)> {
        Vec::new()
    }
    fn lt_lhs_any_rhs_val(&self, _val_r: &Value) -> Vec<(Value, PodRef)> {
        Vec::new()
    }
    fn lt_all_val_val(&self) -> Vec<(Value, Value, PodRef)> {
        Vec::new()
    }

    fn value_of_ak(&self, _root: &Hash, _key: &Key) -> Option<Value> {
        None
    }

    /// Returns the provenance for a Contains(root,key,value) fact if known.
    fn contains_source(&self, _root: &Hash, _key: &Key, _val: &Value) -> Option<ContainsSource> {
        None
    }

    /// Enumerate roots that can justify Contains(root,key,val) along with their provenance.
    fn enumerate_contains_sources(&self, _key: &Key, _val: &Value) -> Vec<(Hash, ContainsSource)> {
        Vec::new()
    }

    /// CopyContains support: list copied values for (root,key).
    fn contains_copied_values(&self, _root: &Hash, _key: &Key) -> Vec<(Value, PodRef)> {
        Vec::new()
    }

    /// ContainsFromEntries support: get a value only if it comes from a full dictionary (generation).
    fn contains_full_value(&self, _root: &Hash, _key: &Key) -> Option<Value> {
        None
    }

    /// Enumerate SumOf rows for CopySumOf (MVP helper).
    fn sumof_rows(&self) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }

    // LtEq copy helpers (parallel to Lt)
    fn lte_lhs_ak_rhs_val(&self, _key: &Key, _val: &Value) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }
    fn lte_lhs_val_rhs_ak(&self, _val: &Value, _key: &Key) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }
    fn lte_ak_ak_by_keys(&self, _left_key: &Key, _right_key: &Key) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }
    fn lte_lhs_ak_rhs_any(&self, _root: &Hash, _key: &Key) -> Vec<(Value, PodRef)> {
        Vec::new()
    }
    fn lte_lhs_any_rhs_ak(&self, _root: &Hash, _key: &Key) -> Vec<(Value, PodRef)> {
        Vec::new()
    }
    fn lte_lhs_val_rhs_val(&self, _val_l: &Value, _val_r: &Value) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }
    fn lte_lhs_val_rhs_any(&self, _val_l: &Value) -> Vec<(Value, PodRef)> {
        Vec::new()
    }
    fn lte_lhs_any_rhs_val(&self, _val_r: &Value) -> Vec<(Value, PodRef)> {
        Vec::new()
    }
    fn lte_all_val_val(&self) -> Vec<(Value, Value, PodRef)> {
        Vec::new()
    }

    // NotContains helpers
    fn not_contains_copy_root_key(&self, _root: &Hash, _key: &Key) -> Option<PodRef> {
        None
    }
    fn not_contains_roots_for_key(&self, _key: &Key) -> Vec<(Hash, PodRef)> {
        Vec::new()
    }
    /// If we know the full dictionary for `root`, return Some(true) if key absent, Some(false) if present, None if unknown.
    fn full_dict_absence(&self, _root: &Hash, _key: &Key) -> Option<bool> {
        None
    }
}

/// Trivial empty EDB for scaffolding.
pub struct EmptyEdb;
impl EdbView for EmptyEdb {}

/// A simple in-memory EDB mock with fixtures for Equal and Contains.
#[derive(Default)]
pub struct MockEdbView {
    /// Equal(AK(root,key), value) rows available to copy.
    pub equal_rows: Vec<(Statement, PodRef)>,
    /// Lt rows available to copy.
    pub lt_rows: Vec<(Statement, PodRef)>,
    /// LtEq rows available to copy.
    pub lte_rows: Vec<(Statement, PodRef)>,
    /// Copied Contains facts: (root, key_hash) -> Vec<(value, PodRef)>
    pub contains_copied: HashMap<(Hash, Hash), Vec<(Value, PodRef)>>,
    /// Full dictionaries registered: root -> key_hash -> value
    pub full_dicts: HashMap<Hash, HashMap<Hash, Value>>,
    /// SumOf rows available to copy.
    pub sum_rows: Vec<(Statement, PodRef)>,
    /// NotContains copied rows: Statement::NotContains(root,key)
    pub not_contains_rows: Vec<(Statement, PodRef)>,
}

fn key_hash(k: &Key) -> Hash {
    k.hash()
}

impl MockEdbView {
    pub fn add_equal_row(
        &mut self,
        root: pod2::middleware::Hash,
        key: Key,
        val: Value,
        src: PodRef,
    ) {
        let st = Statement::Equal(
            ValueRef::Key(AnchoredKey::new(root, key)),
            ValueRef::Literal(val),
        );
        self.equal_rows.push((st, src));
    }
    pub fn add_not_contains_row(&mut self, root: Hash, key: Key, src: PodRef) {
        let st = Statement::NotContains(
            ValueRef::Literal(Value::from(root)),
            ValueRef::Literal(Value::from(key.name())),
        );
        self.not_contains_rows.push((st, src));
    }
    /// Register a copied Contains fact from a pod source.
    pub fn add_copied_contains(&mut self, root: Hash, key: Key, val: Value, src: PodRef) {
        self.contains_copied
            .entry((root, key_hash(&key)))
            .or_default()
            .push((val, src));
    }
    /// Register a full dictionary entry allowing generation of Contains facts.
    pub fn add_full_kv(&mut self, root: Hash, key: Key, val: Value) {
        self.full_dicts
            .entry(root)
            .or_default()
            .insert(key_hash(&key), val);
    }

    /// Register an entire full dictionary (all keys available for GeneratedContains).
    pub fn add_full_dict(&mut self, dict: Dictionary) {
        let root = dict.commitment();
        let entry = self.full_dicts.entry(root).or_default();
        for (k, v) in dict.kvs().iter() {
            entry.insert(k.hash(), v.clone());
        }
    }

    pub fn add_lt_row_lak_rval(
        &mut self,
        root: pod2::middleware::Hash,
        key: Key,
        val: Value,
        src: PodRef,
    ) {
        let st = Statement::Lt(
            ValueRef::Key(AnchoredKey::new(root, key)),
            ValueRef::Literal(val),
        );
        self.lt_rows.push((st, src));
    }
    pub fn add_lt_row_lval_rak(
        &mut self,
        val: Value,
        root: pod2::middleware::Hash,
        key: Key,
        src: PodRef,
    ) {
        let st = Statement::Lt(
            ValueRef::Literal(val),
            ValueRef::Key(AnchoredKey::new(root, key)),
        );
        self.lt_rows.push((st, src));
    }
    pub fn add_lt_row_vals(&mut self, vl: Value, vr: Value, src: PodRef) {
        let st = Statement::Lt(ValueRef::Literal(vl), ValueRef::Literal(vr));
        self.lt_rows.push((st, src));
    }
    pub fn add_lte_row_lak_rval(
        &mut self,
        root: pod2::middleware::Hash,
        key: Key,
        val: Value,
        src: PodRef,
    ) {
        let st = Statement::LtEq(
            ValueRef::Key(AnchoredKey::new(root, key)),
            ValueRef::Literal(val),
        );
        self.lte_rows.push((st, src));
    }
    pub fn add_lte_row_lval_rak(
        &mut self,
        val: Value,
        root: pod2::middleware::Hash,
        key: Key,
        src: PodRef,
    ) {
        let st = Statement::LtEq(
            ValueRef::Literal(val),
            ValueRef::Key(AnchoredKey::new(root, key)),
        );
        self.lte_rows.push((st, src));
    }
    pub fn add_lte_row_vals(&mut self, vl: Value, vr: Value, src: PodRef) {
        let st = Statement::LtEq(ValueRef::Literal(vl), ValueRef::Literal(vr));
        self.lte_rows.push((st, src));
    }

    /// Register a SumOf row for copying.
    pub fn add_sum_row_vals(&mut self, a: Value, b: Value, c: Value, src: PodRef) {
        let st = Statement::SumOf(
            ValueRef::Literal(a),
            ValueRef::Literal(b),
            ValueRef::Literal(c),
        );
        self.sum_rows.push((st, src));
    }
    pub fn add_sum_row_ak_val_val(
        &mut self,
        root: Hash,
        key: Key,
        b: Value,
        c: Value,
        src: PodRef,
    ) {
        let st = Statement::SumOf(
            ValueRef::Key(AnchoredKey::new(root, key)),
            ValueRef::Literal(b),
            ValueRef::Literal(c),
        );
        self.sum_rows.push((st, src));
    }
    pub fn add_sum_row_val_ak_val(
        &mut self,
        a: Value,
        root: Hash,
        key: Key,
        c: Value,
        src: PodRef,
    ) {
        let st = Statement::SumOf(
            ValueRef::Literal(a),
            ValueRef::Key(AnchoredKey::new(root, key)),
            ValueRef::Literal(c),
        );
        self.sum_rows.push((st, src));
    }
    pub fn add_sum_row_val_val_ak(
        &mut self,
        a: Value,
        b: Value,
        root: Hash,
        key: Key,
        src: PodRef,
    ) {
        let st = Statement::SumOf(
            ValueRef::Literal(a),
            ValueRef::Literal(b),
            ValueRef::Key(AnchoredKey::new(root, key)),
        );
        self.sum_rows.push((st, src));
    }
}

impl EdbView for MockEdbView {
    fn match_equal_lhs_ak_rhs_val(&self, key: &Key, val: &Value) -> Vec<(Statement, PodRef)> {
        self.equal_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::Equal(
                    ValueRef::Key(AnchoredKey { key: k, .. }),
                    ValueRef::Literal(v),
                ) => k.hash() == key.hash() && v == val,
                _ => false,
            })
            .cloned()
            .collect()
    }

    fn contains_value(&self, root: &Hash, key: &Key) -> Option<Value> {
        if let Some(vs) = self.contains_copied.get(&(*root, key_hash(key))) {
            if let Some((v, _)) = vs.first() {
                return Some(v.clone());
            }
        }
        self.full_dicts
            .get(root)
            .and_then(|m| m.get(&key_hash(key)).cloned())
    }

    fn roots_with_key_value(&self, key: &Key, val: &Value) -> Vec<Hash> {
        let mut roots: HashSet<Hash> = HashSet::new();
        // From Contains facts
        for ((root, k), vs) in self.contains_copied.iter() {
            if *k == key_hash(key) && vs.iter().any(|(v, _)| v == val) {
                roots.insert(*root);
            }
        }
        // From Equal rows
        for (st, _src) in self.equal_rows.iter() {
            if let Statement::Equal(
                ValueRef::Key(AnchoredKey { root, key: k }),
                ValueRef::Literal(v),
            ) = st
            {
                if k.hash() == key.hash() && v == val {
                    roots.insert(*root);
                }
            }
        }
        // From full dicts
        for (root, kvs) in self.full_dicts.iter() {
            if let Some(v) = kvs.get(&key_hash(key)) {
                if v == val {
                    roots.insert(*root);
                }
            }
        }
        let mut v: Vec<Hash> = roots.into_iter().collect();
        v.sort();
        v
    }

    fn equal_lhs_val_rhs_ak(&self, val: &Value, key: &Key) -> Vec<(Statement, PodRef)> {
        self.equal_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::Equal(
                    ValueRef::Literal(v),
                    ValueRef::Key(AnchoredKey { key: k, .. }),
                ) => v == val && k.hash() == key.hash(),
                _ => false,
            })
            .cloned()
            .collect()
    }

    fn equal_lhs_ak_rhs_any(&self, root: &Hash, key: &Key) -> Vec<(Value, PodRef)> {
        self.equal_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::Equal(
                    ValueRef::Key(AnchoredKey { root: r, key: k }),
                    ValueRef::Literal(v),
                ) if r == root && k.hash() == key.hash() => Some((v.clone(), src.clone())),
                _ => None,
            })
            .collect()
    }

    fn equal_ak_ak_by_keys(&self, left_key: &Key, right_key: &Key) -> Vec<(Statement, PodRef)> {
        self.equal_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::Equal(
                    ValueRef::Key(AnchoredKey { key: lk, .. }),
                    ValueRef::Key(AnchoredKey { key: rk, .. }),
                ) => lk.hash() == left_key.hash() && rk.hash() == right_key.hash(),
                _ => false,
            })
            .cloned()
            .collect()
    }

    fn equal_lhs_any_rhs_ak(&self, root: &Hash, key: &Key) -> Vec<(Value, PodRef)> {
        self.equal_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::Equal(
                    ValueRef::Literal(v),
                    ValueRef::Key(AnchoredKey { root: r, key: k }),
                ) if r == root && k.hash() == key.hash() => Some((v.clone(), src.clone())),
                _ => None,
            })
            .collect()
    }

    // Lt support
    fn lt_lhs_ak_rhs_val(&self, key: &Key, val: &Value) -> Vec<(Statement, PodRef)> {
        self.lt_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::Lt(ValueRef::Key(AnchoredKey { key: k, .. }), ValueRef::Literal(v)) => {
                    k.hash() == key.hash() && v == val
                }
                _ => false,
            })
            .cloned()
            .collect()
    }
    fn lt_lhs_val_rhs_ak(&self, val: &Value, key: &Key) -> Vec<(Statement, PodRef)> {
        self.lt_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::Lt(ValueRef::Literal(v), ValueRef::Key(AnchoredKey { key: k, .. })) => {
                    v == val && k.hash() == key.hash()
                }
                _ => false,
            })
            .cloned()
            .collect()
    }
    fn lt_ak_ak_by_keys(&self, left_key: &Key, right_key: &Key) -> Vec<(Statement, PodRef)> {
        self.lt_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::Lt(
                    ValueRef::Key(AnchoredKey { key: lk, .. }),
                    ValueRef::Key(AnchoredKey { key: rk, .. }),
                ) => lk.hash() == left_key.hash() && rk.hash() == right_key.hash(),
                _ => false,
            })
            .cloned()
            .collect()
    }
    fn lt_lhs_ak_rhs_any(&self, root: &Hash, key: &Key) -> Vec<(Value, PodRef)> {
        self.lt_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::Lt(
                    ValueRef::Key(AnchoredKey { root: r, key: k }),
                    ValueRef::Literal(v),
                ) if r == root && k.hash() == key.hash() => Some((v.clone(), src.clone())),
                _ => None,
            })
            .collect()
    }
    fn lt_lhs_any_rhs_ak(&self, root: &Hash, key: &Key) -> Vec<(Value, PodRef)> {
        self.lt_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::Lt(
                    ValueRef::Literal(v),
                    ValueRef::Key(AnchoredKey { root: r, key: k }),
                ) if r == root && k.hash() == key.hash() => Some((v.clone(), src.clone())),
                _ => None,
            })
            .collect()
    }

    fn lt_lhs_val_rhs_val(&self, val_l: &Value, val_r: &Value) -> Vec<(Statement, PodRef)> {
        self.lt_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::Lt(ValueRef::Literal(vl), ValueRef::Literal(vr)) => {
                    vl == val_l && vr == val_r
                }
                _ => false,
            })
            .cloned()
            .collect()
    }
    fn lt_lhs_val_rhs_any(&self, val_l: &Value) -> Vec<(Value, PodRef)> {
        self.lt_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::Lt(ValueRef::Literal(vl), ValueRef::Literal(vr)) if vl == val_l => {
                    Some((vr.clone(), src.clone()))
                }
                _ => None,
            })
            .collect()
    }
    fn lt_lhs_any_rhs_val(&self, val_r: &Value) -> Vec<(Value, PodRef)> {
        self.lt_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::Lt(ValueRef::Literal(vl), ValueRef::Literal(vr)) if vr == val_r => {
                    Some((vl.clone(), src.clone()))
                }
                _ => None,
            })
            .collect()
    }
    fn lt_all_val_val(&self) -> Vec<(Value, Value, PodRef)> {
        self.lt_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::Lt(ValueRef::Literal(vl), ValueRef::Literal(vr)) => {
                    Some((vl.clone(), vr.clone(), src.clone()))
                }
                _ => None,
            })
            .collect()
    }

    // LtEq support
    fn lte_lhs_ak_rhs_val(&self, key: &Key, val: &Value) -> Vec<(Statement, PodRef)> {
        self.lte_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::LtEq(
                    ValueRef::Key(AnchoredKey { key: k, .. }),
                    ValueRef::Literal(v),
                ) => k.hash() == key.hash() && v == val,
                _ => false,
            })
            .cloned()
            .collect()
    }
    fn lte_lhs_val_rhs_ak(&self, val: &Value, key: &Key) -> Vec<(Statement, PodRef)> {
        self.lte_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::LtEq(
                    ValueRef::Literal(v),
                    ValueRef::Key(AnchoredKey { key: k, .. }),
                ) => v == val && k.hash() == key.hash(),
                _ => false,
            })
            .cloned()
            .collect()
    }
    fn lte_ak_ak_by_keys(&self, left_key: &Key, right_key: &Key) -> Vec<(Statement, PodRef)> {
        self.lte_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::LtEq(
                    ValueRef::Key(AnchoredKey { key: lk, .. }),
                    ValueRef::Key(AnchoredKey { key: rk, .. }),
                ) => lk.hash() == left_key.hash() && rk.hash() == right_key.hash(),
                _ => false,
            })
            .cloned()
            .collect()
    }
    fn lte_lhs_ak_rhs_any(&self, root: &Hash, key: &Key) -> Vec<(Value, PodRef)> {
        self.lte_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::LtEq(
                    ValueRef::Key(AnchoredKey { root: r, key: k }),
                    ValueRef::Literal(v),
                ) if r == root && k.hash() == key.hash() => Some((v.clone(), src.clone())),
                _ => None,
            })
            .collect()
    }
    fn lte_lhs_any_rhs_ak(&self, root: &Hash, key: &Key) -> Vec<(Value, PodRef)> {
        self.lte_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::LtEq(
                    ValueRef::Literal(v),
                    ValueRef::Key(AnchoredKey { root: r, key: k }),
                ) if r == root && k.hash() == key.hash() => Some((v.clone(), src.clone())),
                _ => None,
            })
            .collect()
    }
    fn lte_lhs_val_rhs_val(&self, val_l: &Value, val_r: &Value) -> Vec<(Statement, PodRef)> {
        self.lte_rows
            .iter()
            .filter(|(st, _)| match st {
                Statement::LtEq(ValueRef::Literal(vl), ValueRef::Literal(vr)) => {
                    vl == val_l && vr == val_r
                }
                _ => false,
            })
            .cloned()
            .collect()
    }
    fn lte_lhs_val_rhs_any(&self, val_l: &Value) -> Vec<(Value, PodRef)> {
        self.lte_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::LtEq(ValueRef::Literal(vl), ValueRef::Literal(vr)) if vl == val_l => {
                    Some((vr.clone(), src.clone()))
                }
                _ => None,
            })
            .collect()
    }
    fn lte_lhs_any_rhs_val(&self, val_r: &Value) -> Vec<(Value, PodRef)> {
        self.lte_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::LtEq(ValueRef::Literal(vl), ValueRef::Literal(vr)) if vr == val_r => {
                    Some((vl.clone(), src.clone()))
                }
                _ => None,
            })
            .collect()
    }
    fn lte_all_val_val(&self) -> Vec<(Value, Value, PodRef)> {
        self.lte_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::LtEq(ValueRef::Literal(vl), ValueRef::Literal(vr)) => {
                    Some((vl.clone(), vr.clone(), src.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn value_of_ak(&self, root: &Hash, key: &Key) -> Option<Value> {
        // Prefer Contains fact
        if let Some(v) = self.contains_value(root, key) {
            return Some(v);
        }
        // Fall back to Equal rows
        for (st, _src) in self.equal_rows.iter() {
            if let Statement::Equal(
                ValueRef::Key(AnchoredKey { root: r, key: k }),
                ValueRef::Literal(v),
            ) = st
            {
                if r == root && k.hash() == key.hash() {
                    return Some(v.clone());
                }
            }
        }
        None
    }

    fn contains_source(&self, root: &Hash, key: &Key, val: &Value) -> Option<ContainsSource> {
        if let Some(vs) = self.contains_copied.get(&(*root, key_hash(key))) {
            for (v, pod) in vs.iter() {
                if v == val {
                    return Some(ContainsSource::Copied { pod: pod.clone() });
                }
            }
        }
        if let Some(kvs) = self.full_dicts.get(root) {
            if let Some(v) = kvs.get(&key_hash(key)) {
                if v == val {
                    return Some(ContainsSource::GeneratedFromFullDict { root: *root });
                }
            }
        }
        None
    }

    fn enumerate_contains_sources(&self, key: &Key, val: &Value) -> Vec<(Hash, ContainsSource)> {
        let mut out = Vec::new();
        for ((root, k), vs) in self.contains_copied.iter() {
            if *k == key_hash(key) {
                for (v, pod) in vs.iter() {
                    if v == val {
                        out.push((*root, ContainsSource::Copied { pod: pod.clone() }));
                    }
                }
            }
        }
        for (root, kvs) in self.full_dicts.iter() {
            if let Some(v) = kvs.get(&key_hash(key)) {
                if v == val {
                    out.push((*root, ContainsSource::GeneratedFromFullDict { root: *root }));
                }
            }
        }
        out.sort_by(|(r1, s1), (r2, s2)| {
            r1.cmp(r2).then_with(|| match (s1, s2) {
                (ContainsSource::GeneratedFromFullDict { .. }, ContainsSource::Copied { .. }) => {
                    std::cmp::Ordering::Less
                }
                (ContainsSource::Copied { .. }, ContainsSource::GeneratedFromFullDict { .. }) => {
                    std::cmp::Ordering::Greater
                }
                _ => std::cmp::Ordering::Equal,
            })
        });
        out
    }

    fn contains_copied_values(&self, root: &Hash, key: &Key) -> Vec<(Value, PodRef)> {
        self.contains_copied
            .get(&(*root, key_hash(key)))
            .cloned()
            .unwrap_or_else(Vec::new)
    }

    fn contains_full_value(&self, root: &Hash, key: &Key) -> Option<Value> {
        self.full_dicts
            .get(root)
            .and_then(|m| m.get(&key_hash(key)).cloned())
    }

    fn sumof_rows(&self) -> Vec<(Statement, PodRef)> {
        self.sum_rows.clone()
    }

    // NotContains
    fn not_contains_copy_root_key(&self, root: &Hash, key: &Key) -> Option<PodRef> {
        self.not_contains_rows
            .iter()
            .find_map(|(st, src)| match st {
                Statement::NotContains(ValueRef::Literal(r), ValueRef::Literal(k)) => {
                    if Hash::from(r.raw()) == *root && k == &Value::from(key.name()) {
                        Some(src.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
    }
    fn not_contains_roots_for_key(&self, key: &Key) -> Vec<(Hash, PodRef)> {
        self.not_contains_rows
            .iter()
            .filter_map(|(st, src)| match st {
                Statement::NotContains(ValueRef::Literal(r), ValueRef::Literal(k)) => {
                    if k == &Value::from(key.name()) {
                        Some((Hash::from(r.raw()), src.clone()))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect()
    }
    fn full_dict_absence(&self, root: &Hash, key: &Key) -> Option<bool> {
        self.full_dicts
            .get(root)
            .map(|map| !map.contains_key(&key_hash(key)))
    }
}

/// Provenance of a Contains(root,key,value) fact.
#[derive(Clone, Debug)]
pub enum ContainsSource {
    Copied { pod: PodRef },
    GeneratedFromFullDict { root: Hash },
}
