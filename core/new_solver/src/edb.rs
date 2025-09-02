use std::collections::HashMap;

use pod2::{
    frontend::{MainPod, SignedDict},
    middleware::{
        containers::Dictionary, AnchoredKey, CustomPredicateRef, Hash, Key, Statement, Value,
        ValueRef,
    },
};

use crate::types::{ConstraintStore, OpTag, PodRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryPred {
    Equal,
    Lt,
    LtEq,
    SignedBy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TernaryPred {
    SumOf,
    ProductOf,
    MaxOf,
    HashOf,
    Contains,
}

#[derive(Clone, Copy, Debug)]
pub enum ArgSel<'a> {
    /// Match a literal value exactly
    Literal(&'a pod2::middleware::Value),
    /// Match any literal value
    Val,
    /// Match an anchored key by its key only (any root)
    AkByKey(&'a pod2::middleware::Key),
    /// Match an anchored key by exact root and key
    AkExact {
        root: &'a pod2::middleware::Hash,
        key: &'a pod2::middleware::Key,
    },
}

/// Minimal read-only EDB interface for OpHandlers in MVP.
pub trait EdbView: Send + Sync {
    /// Generic binary predicate query. Implementors should override this; all exact wrappers delegate here.
    fn query_binary(
        &self,
        _pred: BinaryPred,
        _lhs: ArgSel,
        _rhs: ArgSel,
    ) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }

    /// Generic ternary predicate query. Implementors can override as needed.
    fn query_ternary(
        &self,
        _pred: TernaryPred,
        _a: ArgSel,
        _b: ArgSel,
        _c: ArgSel,
    ) -> Vec<(Statement, PodRef)> {
        Vec::new()
    }

    fn contains_value(&self, _root: &pod2::middleware::Hash, _key: &Key) -> Option<Value> {
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

    /// Enumerate existing custom heads matching the literal mask.
    /// `filters[i] = Some(v)` requires head arg i == v; `None` matches any.
    fn custom_matches(
        &self,
        _pred: &CustomPredicateRef,
        _filters: &[Option<Value>],
    ) -> Vec<(Vec<Value>, PodRef)> {
        Vec::new()
    }

    /// Convenience predicate: true if at least one custom head matches the filter mask.
    fn custom_any_match(&self, pred: &CustomPredicateRef, filters: &[Option<Value>]) -> bool {
        !self.custom_matches(pred, filters).is_empty()
    }

    /// Lookup a SignedDict by its root commitment, if tracked by the EDB.
    fn signed_dict(&self, _root: &Hash) -> Option<SignedDict> {
        None
    }

    /// Lookup a full Dictionary by its root commitment, if tracked by the EDB.
    fn full_dict(&self, _root: &Hash) -> Option<Dictionary> {
        None
    }

    /// Enumerate all SignedDicts tracked by the EDB (used for generation/enumeration).
    fn enumerate_signed_dicts(&self) -> Vec<SignedDict> {
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

    /// Resolve a stored MainPod by its PodRef, if available.
    fn resolve_pod(&self, _id: &PodRef) -> Option<MainPod> {
        None
    }

    /// Compute the minimal set of PodRefs required to justify Copy-style proofs in an answer.
    fn required_pods_for_answer(
        &self,
        _ans: &ConstraintStore,
    ) -> std::collections::BTreeSet<PodRef> {
        std::collections::BTreeSet::new()
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
    /// Original full dictionary objects by root (used for replay)
    pub full_dict_objs: HashMap<Hash, Dictionary>,
    /// SumOf rows available to copy.
    pub sum_rows: Vec<(Statement, PodRef)>,
    /// ProductOf rows available to copy.
    pub product_rows: Vec<(Statement, PodRef)>,
    /// MaxOf rows available to copy.
    pub max_rows: Vec<(Statement, PodRef)>,
    /// HashOf rows available to copy.
    pub hash_rows: Vec<(Statement, PodRef)>,
    /// SignedBy rows available to copy.
    pub signed_by_rows: Vec<(Statement, PodRef)>,
    /// NotContains copied rows: Statement::NotContains(root,key)
    pub not_contains_rows: Vec<(Statement, PodRef)>,
    /// Signed dictionaries indexed by their root commitment
    pub signed_dicts: HashMap<Hash, SignedDict>,
    /// Custom statement rows: predicate key (batch id + index) -> list of (args, PodRef)
    custom_rows: std::collections::BTreeMap<CprKey, Vec<(Vec<Value>, PodRef)>>,
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
        self.full_dict_objs.insert(root, dict.clone());
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

    pub fn add_hash_row(&mut self, a: Value, b: Value, c: Value, src: PodRef) {
        let st = Statement::HashOf(
            ValueRef::Literal(a),
            ValueRef::Literal(b),
            ValueRef::Literal(c),
        );
        self.hash_rows.push((st, src));
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

    /// Register a ProductOf row for copying.
    pub fn add_product_row_vals(&mut self, a: Value, b: Value, c: Value, src: PodRef) {
        let st = Statement::ProductOf(
            ValueRef::Literal(a),
            ValueRef::Literal(b),
            ValueRef::Literal(c),
        );
        self.product_rows.push((st, src));
    }
    pub fn add_product_row_ak_val_val(
        &mut self,
        root: Hash,
        key: Key,
        b: Value,
        c: Value,
        src: PodRef,
    ) {
        let st = Statement::ProductOf(
            ValueRef::Key(AnchoredKey::new(root, key)),
            ValueRef::Literal(b),
            ValueRef::Literal(c),
        );
        self.product_rows.push((st, src));
    }
    pub fn add_product_row_val_ak_val(
        &mut self,
        a: Value,
        root: Hash,
        key: Key,
        c: Value,
        src: PodRef,
    ) {
        let st = Statement::ProductOf(
            ValueRef::Literal(a),
            ValueRef::Key(AnchoredKey::new(root, key)),
            ValueRef::Literal(c),
        );
        self.product_rows.push((st, src));
    }
    pub fn add_product_row_val_val_ak(
        &mut self,
        a: Value,
        b: Value,
        root: Hash,
        key: Key,
        src: PodRef,
    ) {
        let st = Statement::ProductOf(
            ValueRef::Literal(a),
            ValueRef::Literal(b),
            ValueRef::Key(AnchoredKey::new(root, key)),
        );
        self.product_rows.push((st, src));
    }

    /// Register a MaxOf row for copying.
    pub fn add_max_row_vals(&mut self, a: Value, b: Value, c: Value, src: PodRef) {
        let st = Statement::MaxOf(
            ValueRef::Literal(a),
            ValueRef::Literal(b),
            ValueRef::Literal(c),
        );
        self.max_rows.push((st, src));
    }
    pub fn add_max_row_ak_val_val(
        &mut self,
        root: Hash,
        key: Key,
        b: Value,
        c: Value,
        src: PodRef,
    ) {
        let st = Statement::MaxOf(
            ValueRef::Key(AnchoredKey::new(root, key)),
            ValueRef::Literal(b),
            ValueRef::Literal(c),
        );
        self.max_rows.push((st, src));
    }
    pub fn add_max_row_val_ak_val(
        &mut self,
        a: Value,
        root: Hash,
        key: Key,
        c: Value,
        src: PodRef,
    ) {
        let st = Statement::MaxOf(
            ValueRef::Literal(a),
            ValueRef::Key(AnchoredKey::new(root, key)),
            ValueRef::Literal(c),
        );
        self.max_rows.push((st, src));
    }
    pub fn add_max_row_val_val_ak(
        &mut self,
        a: Value,
        b: Value,
        root: Hash,
        key: Key,
        src: PodRef,
    ) {
        let st = Statement::MaxOf(
            ValueRef::Literal(a),
            ValueRef::Literal(b),
            ValueRef::Key(AnchoredKey::new(root, key)),
        );
        self.max_rows.push((st, src));
    }

    /// Register a ground custom head tuple with provenance.
    pub fn add_custom_row(&mut self, pred: CustomPredicateRef, args: Vec<Value>, src: PodRef) {
        self.custom_rows
            .entry(CprKey::from(&pred))
            .or_default()
            .push((args, src));
    }

    /// Register a SignedDict; also index its full dictionary for GeneratedContains.
    pub fn add_signed_dict(&mut self, signed: SignedDict) {
        let root = signed.dict.commitment();
        self.signed_dicts.insert(root, signed.clone());
        self.add_full_dict(signed.dict);
    }

    fn matches_arg<'a>(vr: &pod2::middleware::ValueRef, sel: &ArgSel<'a>) -> bool {
        use pod2::middleware::{AnchoredKey, ValueRef};
        match sel {
            ArgSel::Literal(v) => matches!(vr, ValueRef::Literal(v0) if v0 == *v),
            ArgSel::Val => matches!(vr, ValueRef::Literal(_)),
            ArgSel::AkByKey(key) => {
                matches!(vr, ValueRef::Key(AnchoredKey { key: k, .. }) if k.hash() == key.hash())
            }
            ArgSel::AkExact { root, key } => {
                matches!(vr, ValueRef::Key(AnchoredKey { root: r, key: k }) if r == *root && k.hash() == key.hash())
            }
        }
    }
}

impl EdbView for MockEdbView {
    fn query_binary(&self, pred: BinaryPred, lhs: ArgSel, rhs: ArgSel) -> Vec<(Statement, PodRef)> {
        use pod2::middleware::Statement::*;
        let rows: &Vec<(Statement, PodRef)> = match pred {
            BinaryPred::Equal => &self.equal_rows,
            BinaryPred::Lt => &self.lt_rows,
            BinaryPred::LtEq => &self.lte_rows,
            BinaryPred::SignedBy => &self.signed_by_rows,
        };
        rows.iter()
            .filter(|(st, _)| match st {
                Equal(l, r) | Lt(l, r) | LtEq(l, r) => {
                    Self::matches_arg(l, &lhs) && Self::matches_arg(r, &rhs)
                }
                _ => false,
            })
            .cloned()
            .collect()
    }

    fn custom_matches(
        &self,
        pred: &CustomPredicateRef,
        filters: &[Option<Value>],
    ) -> Vec<(Vec<Value>, PodRef)> {
        let rows = match self.custom_rows.get(&CprKey::from(pred)) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        'row: for (args, src) in rows.iter() {
            if args.len() != filters.len() {
                continue;
            }
            for (a, f) in args.iter().zip(filters.iter()) {
                if let Some(v) = f {
                    if a.raw() != v.raw() {
                        continue 'row;
                    }
                }
            }
            out.push((args.clone(), src.clone()));
        }
        out
    }
    fn query_ternary(
        &self,
        pred: TernaryPred,
        a: ArgSel,
        b: ArgSel,
        c: ArgSel,
    ) -> Vec<(Statement, PodRef)> {
        use pod2::middleware::Statement::*;
        let rows: &Vec<(Statement, PodRef)> = match pred {
            TernaryPred::SumOf => &self.sum_rows,
            TernaryPred::ProductOf => &self.product_rows,
            TernaryPred::MaxOf => &self.max_rows,
            TernaryPred::HashOf => &self.hash_rows,
            // Contains rows are materialized via contains_* helpers; skip here.
            TernaryPred::Contains => return Vec::new(),
        };
        rows.iter()
            .filter(|(st, _)| match st {
                SumOf(la, lb, lc)
                | ProductOf(la, lb, lc)
                | MaxOf(la, lb, lc)
                | HashOf(la, lb, lc) => {
                    Self::matches_arg(la, &a)
                        && Self::matches_arg(lb, &b)
                        && Self::matches_arg(lc, &c)
                }
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

    fn signed_dict(&self, root: &Hash) -> Option<SignedDict> {
        self.signed_dicts.get(root).cloned()
    }

    fn full_dict(&self, root: &Hash) -> Option<Dictionary> {
        self.full_dict_objs.get(root).cloned()
    }

    fn enumerate_signed_dicts(&self) -> Vec<SignedDict> {
        self.signed_dicts.values().cloned().collect()
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

/// Immutable, deterministically ordered EDB built from pods and/or signed dictionaries.
#[derive(Default, Clone)]
pub struct ImmutableEdb {
    // CopyEqual rows
    equal_rows: Vec<(Statement, PodRef)>,
    // Lt and LtEq copied rows
    lt_rows: Vec<(Statement, PodRef)>,
    lte_rows: Vec<(Statement, PodRef)>,
    // Copied Contains facts: (root, key_hash) -> Vec<(value, PodRef)>
    contains_copied: std::collections::BTreeMap<(Hash, Hash), Vec<(Value, PodRef)>>,
    // Full dictionaries registered: root -> key_hash -> value
    full_dicts: std::collections::BTreeMap<Hash, std::collections::BTreeMap<Hash, Value>>,
    // Original full dictionary objects by root (used for replay)
    full_dict_objs: std::collections::BTreeMap<Hash, Dictionary>,
    // Optional copied rows for other predicates (kept for parity/extension)
    not_contains_rows: Vec<(Statement, PodRef)>,
    sum_rows: Vec<(Statement, PodRef)>,
    product_rows: Vec<(Statement, PodRef)>,
    max_rows: Vec<(Statement, PodRef)>,
    hash_rows: Vec<(Statement, PodRef)>,
    signed_by_rows: Vec<(Statement, PodRef)>,
    signed_dicts: std::collections::BTreeMap<Hash, SignedDict>,
    // Custom statement rows: predicate key (batch id + index) -> list of (args, PodRef)
    custom_rows: std::collections::BTreeMap<CprKey, Vec<(Vec<Value>, PodRef)>>,
    // Stored pods by id for replay
    pods: std::collections::BTreeMap<PodRef, MainPod>,
}

/// Ordered key for indexing CustomPredicateRef by (batch_id, index)
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CprKey {
    batch_id: Hash,
    index: usize,
}

impl From<&CustomPredicateRef> for CprKey {
    fn from(cpr: &CustomPredicateRef) -> Self {
        Self {
            batch_id: cpr.batch.id(),
            index: cpr.index,
        }
    }
}

pub struct ImmutableEdbBuilder {
    inner: ImmutableEdb,
}

#[allow(clippy::derivable_impls)]
impl Default for ImmutableEdbBuilder {
    fn default() -> Self {
        Self {
            inner: ImmutableEdb::default(),
        }
    }
}

impl ImmutableEdbBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_full_kv(mut self, root: Hash, key: Key, val: Value) -> Self {
        self.inner
            .full_dicts
            .entry(root)
            .or_default()
            .insert(key_hash(&key), val);
        self
    }

    pub fn add_full_dict(mut self, dict: Dictionary) -> Self {
        let root = dict.commitment();
        self.inner.full_dict_objs.insert(root, dict.clone());
        let entry = self.inner.full_dicts.entry(root).or_default();
        for (k, v) in dict.kvs().iter() {
            entry.insert(k.hash(), v.clone());
        }
        self
    }

    /// Register a full dictionary that is externally signed. For the EDB, a root is a root;
    /// signing is enforced by separate SignedBy statements. This indexes the dictionary identically
    /// to `add_full_dict` so handlers can generate Contains/Equal-from-entries.
    pub fn add_signed_dict(mut self, signed_dict: SignedDict) -> Self {
        let root = signed_dict.dict.commitment();
        self.inner.signed_dicts.insert(root, signed_dict.clone());
        // Also index full dictionary so entries are available to handlers
        self = self.add_full_dict(signed_dict.dict);
        self
    }

    pub fn build(mut self) -> ImmutableEdb {
        // Canonicalize ordering where applicable
        self.inner
            .equal_rows
            .sort_by(|(a, _), (b, _)| format!("{a:?}").cmp(&format!("{b:?}")));
        self.inner
    }

    /// Ingest a MainPod: store it and index its public statements and dictionaries.
    pub fn add_main_pod(mut self, pod: &MainPod) -> Self {
        use pod2::middleware::{Statement as Stmt, TypedValue};
        let pod_ref = PodRef(pod.id());
        self.inner.pods.insert(pod_ref.clone(), pod.clone());
        for st in pod.public_statements.iter() {
            match st {
                // Equal rows (copyable)
                Stmt::Equal(ValueRef::Key(_), ValueRef::Key(_))
                | Stmt::Equal(ValueRef::Key(_), ValueRef::Literal(_))
                | Stmt::Equal(ValueRef::Literal(_), ValueRef::Key(_)) => {
                    self.inner.equal_rows.push((st.clone(), pod_ref.clone()));
                }
                // Contains rows (copied): Contains(root_hash, key_string, value)
                Stmt::Contains(
                    ValueRef::Literal(r),
                    ValueRef::Literal(k),
                    ValueRef::Literal(v),
                ) => {
                    let root = Hash::from(r.raw());
                    if let TypedValue::String(ks) = k.typed() {
                        let key = Key::from(ks.clone());
                        self.inner
                            .contains_copied
                            .entry((root, key.hash()))
                            .or_default()
                            .push((v.clone(), pod_ref.clone()));
                    }
                }
                // NotContains (copied)
                Stmt::NotContains(ValueRef::Literal(_), ValueRef::Literal(_)) => {
                    self.inner
                        .not_contains_rows
                        .push((st.clone(), pod_ref.clone()));
                }
                // SumOf (copied)
                Stmt::SumOf(_, _, _) => {
                    self.inner.sum_rows.push((st.clone(), pod_ref.clone()));
                }
                // ProductOf (copied)
                Stmt::ProductOf(_, _, _) => {
                    self.inner.product_rows.push((st.clone(), pod_ref.clone()));
                }
                // MaxOf (copied)
                Stmt::MaxOf(_, _, _) => {
                    self.inner.max_rows.push((st.clone(), pod_ref.clone()));
                }
                // HashOf (copied)
                Stmt::HashOf(_, _, _) => {
                    self.inner.hash_rows.push((st.clone(), pod_ref.clone()));
                }
                // Lt/LtEq copied rows
                Stmt::Lt(_, _) => {
                    self.inner.lt_rows.push((st.clone(), pod_ref.clone()));
                }
                Stmt::LtEq(_, _) => {
                    self.inner.lte_rows.push((st.clone(), pod_ref.clone()));
                }
                Stmt::SignedBy(_, _) => {
                    self.inner
                        .signed_by_rows
                        .push((st.clone(), pod_ref.clone()));
                }
                Stmt::Custom(pred, vals) => {
                    let args = vals.clone();
                    self.inner
                        .custom_rows
                        .entry(CprKey::from(pred))
                        .or_default()
                        .push((args, pod_ref.clone()));
                }
                _ => {}
            }

            for arg in st.args() {
                if let pod2::middleware::StatementArg::Literal(v) = arg {
                    if let TypedValue::Dictionary(dict) = v.typed() {
                        self = self.add_full_dict(dict.clone());
                    }
                }
            }
        }
        self
    }
}

impl EdbView for ImmutableEdb {
    fn query_binary(&self, pred: BinaryPred, lhs: ArgSel, rhs: ArgSel) -> Vec<(Statement, PodRef)> {
        use pod2::middleware::Statement::*;
        let rows: &Vec<(Statement, PodRef)> = match pred {
            BinaryPred::Equal => &self.equal_rows,
            BinaryPred::Lt => &self.lt_rows,
            BinaryPred::LtEq => &self.lte_rows,
            BinaryPred::SignedBy => &self.signed_by_rows,
        };
        fn matches<'a>(vr: &pod2::middleware::ValueRef, sel: &ArgSel<'a>) -> bool {
            use pod2::middleware::{AnchoredKey, ValueRef};
            match sel {
                ArgSel::Literal(v) => matches!(vr, ValueRef::Literal(v0) if v0 == *v),
                ArgSel::Val => matches!(vr, ValueRef::Literal(_)),
                ArgSel::AkByKey(key) => {
                    matches!(vr, ValueRef::Key(AnchoredKey { key: k, .. }) if k.hash() == key.hash())
                }
                ArgSel::AkExact { root, key } => {
                    matches!(vr, ValueRef::Key(AnchoredKey { root: r, key: k }) if r == *root && k.hash() == key.hash())
                }
            }
        }
        rows.iter()
            .filter(|(st, _)| match st {
                Equal(l, r) | Lt(l, r) | LtEq(l, r) => matches(l, &lhs) && matches(r, &rhs),
                _ => false,
            })
            .cloned()
            .collect()
    }

    fn custom_matches(
        &self,
        pred: &CustomPredicateRef,
        filters: &[Option<Value>],
    ) -> Vec<(Vec<Value>, PodRef)> {
        let rows = match self.custom_rows.get(&CprKey::from(pred)) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        'row: for (args, src) in rows.iter() {
            if args.len() != filters.len() {
                continue;
            }
            for (a, f) in args.iter().zip(filters.iter()) {
                if let Some(v) = f {
                    if a.raw() != v.raw() {
                        continue 'row;
                    }
                }
            }
            out.push((args.clone(), src.clone()));
        }
        out
    }
    fn query_ternary(
        &self,
        pred: TernaryPred,
        a: ArgSel,
        b: ArgSel,
        c: ArgSel,
    ) -> Vec<(Statement, PodRef)> {
        use pod2::middleware::Statement::*;
        let rows: &Vec<(Statement, PodRef)> = match pred {
            TernaryPred::SumOf => &self.sum_rows,
            TernaryPred::ProductOf => &self.product_rows,
            TernaryPred::MaxOf => &self.max_rows,
            TernaryPred::HashOf => &self.hash_rows,
            // Contains rows are materialized via contains_* helpers; skip here.
            TernaryPred::Contains => return Vec::new(),
        };
        fn matches<'a>(vr: &pod2::middleware::ValueRef, sel: &ArgSel<'a>) -> bool {
            use pod2::middleware::{AnchoredKey, ValueRef};
            match sel {
                ArgSel::Literal(v) => matches!(vr, ValueRef::Literal(v0) if v0 == *v),
                ArgSel::Val => matches!(vr, ValueRef::Literal(_)),
                ArgSel::AkByKey(key) => {
                    matches!(vr, ValueRef::Key(AnchoredKey { key: k, .. }) if k.hash() == key.hash())
                }
                ArgSel::AkExact { root, key } => {
                    matches!(vr, ValueRef::Key(AnchoredKey { root: r, key: k }) if r == *root && k.hash() == key.hash())
                }
            }
        }
        rows.iter()
            .filter(|(st, _)| match st {
                SumOf(la, lb, lc)
                | ProductOf(la, lb, lc)
                | MaxOf(la, lb, lc)
                | HashOf(la, lb, lc) => matches(&la, &a) && matches(&lb, &b) && matches(&lc, &c),
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

    fn contains_source(&self, root: &Hash, key: &Key, val: &Value) -> Option<ContainsSource> {
        if let Some(kvs) = self.full_dicts.get(root) {
            if let Some(v) = kvs.get(&key_hash(key)) {
                if v == val {
                    return Some(ContainsSource::GeneratedFromFullDict { root: *root });
                }
            }
        }
        if let Some(vs) = self.contains_copied.get(&(*root, key_hash(key))) {
            for (v, pod) in vs.iter() {
                if v == val {
                    return Some(ContainsSource::Copied { pod: pod.clone() });
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
                    std::cmp::Ordering::Greater
                }
                (ContainsSource::Copied { .. }, ContainsSource::GeneratedFromFullDict { .. }) => {
                    std::cmp::Ordering::Less
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

    fn signed_dict(&self, root: &Hash) -> Option<SignedDict> {
        self.signed_dicts.get(root).cloned()
    }

    fn full_dict(&self, root: &Hash) -> Option<Dictionary> {
        self.full_dict_objs.get(root).cloned()
    }

    fn enumerate_signed_dicts(&self) -> Vec<SignedDict> {
        self.signed_dicts.values().cloned().collect()
    }

    fn resolve_pod(&self, id: &PodRef) -> Option<MainPod> {
        self.pods.get(id).cloned()
    }

    fn required_pods_for_answer(
        &self,
        ans: &ConstraintStore,
    ) -> std::collections::BTreeSet<PodRef> {
        use std::collections::BTreeSet;
        fn walk(tag: &OpTag, acc: &mut BTreeSet<PodRef>) {
            match tag {
                OpTag::CopyStatement { source } => {
                    acc.insert(source.clone());
                }
                OpTag::Derived { premises } | OpTag::CustomDeduction { premises, .. } => {
                    for (_, t) in premises.iter() {
                        walk(t, acc);
                    }
                }
                _ => {}
            }
        }
        let mut out = BTreeSet::new();
        for (_stmt, tag) in ans.premises.iter() {
            walk(tag, &mut out);
        }
        out
    }
}
