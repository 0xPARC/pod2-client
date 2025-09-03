use std::fmt::Debug;

use pod2::{
    frontend::{MainPod, SignedDict},
    middleware::{
        containers::Dictionary, CustomPredicateRef, Hash, Key, PublicKey, SecretKey, Statement,
        StatementArg, Value, ValueRef,
    },
};

use crate::{types::PodRef, RawOrdValue};

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
pub trait EdbView {
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

    fn contains_value(&self, _root: &pod2::middleware::Hash, _key: &Key) -> Option<Value>;

    /// Returns the provenance for a Contains(root,key,value) fact if known.
    fn contains_source(&self, _root: &Hash, _key: &Key, _val: &Value) -> Option<ContainsSource>;

    /// Enumerate roots that can justify Contains(root,key,val) along with their provenance.
    fn enumerate_contains_sources(&self, _key: &Key, _val: &Value) -> Vec<(Hash, ContainsSource)>;

    /// CopyContains support: list copied values for (root,key).
    fn contains_copied_values(&self, _root: &Hash, _key: &Key) -> Vec<(Value, PodRef)>;

    /// ContainsFromEntries support: get a value only if it comes from a full dictionary (generation).
    fn contains_full_value(&self, _root: &Hash, _key: &Key) -> Option<Value>;

    /// Enumerate existing custom heads matching the literal mask.
    /// `filters[i] = Some(v)` requires head arg i == v; `None` matches any.
    fn custom_matches(
        &self,
        _pred: &CustomPredicateRef,
        _filters: &[Option<Value>],
    ) -> Vec<(Vec<Value>, PodRef)>;

    /// Convenience predicate: true if at least one custom head matches the filter mask.
    fn custom_any_match(&self, pred: &CustomPredicateRef, filters: &[Option<Value>]) -> bool;

    /// Lookup a SignedDict by its root commitment, if tracked by the EDB.
    fn signed_dict(&self, _root: &Hash) -> Option<SignedDict>;

    /// Lookup a full Dictionary by its root commitment, if tracked by the EDB.
    fn full_dict(&self, _root: &Hash) -> Option<Dictionary>;

    /// Enumerate all SignedDicts tracked by the EDB (used for generation/enumeration).
    fn enumerate_signed_dicts(&self) -> Vec<SignedDict>;

    // NotContains helpers
    fn not_contains_copy_root_key(&self, _root: &Hash, _key: &Key) -> Option<PodRef>;
    fn not_contains_roots_for_key(&self, _key: &Key) -> Vec<(Hash, PodRef)>;
    /// If we know the full dictionary for `root`, return Some(true) if key absent, Some(false) if present, None if unknown.
    fn full_dict_absence(&self, _root: &Hash, _key: &Key) -> Option<bool>;

    /// Resolve a stored MainPod by its PodRef, if available.
    fn resolve_pod(&self, _id: &PodRef) -> Option<MainPod>;

    /// Enumerate all keypairs tracked by the EDB (used for generation/enumeration).
    fn enumerate_keypairs(&self) -> Vec<(Value, Value)>;

    fn get_secret_key(&self, _public_key: &PublicKey) -> Option<&SecretKey>;
}

/// Provenance of a Contains(root,key,value) fact.
#[derive(Clone, Debug)]
pub enum ContainsSource {
    Copied { pod: PodRef },
    GeneratedFromFullDict { root: Hash },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum IndexKey {
    Literal(RawOrdValue),
    AnyLiteral,
    FullAnchoredKey(Hash, Hash),
    PartialAnchoredKey(Hash),
}

#[derive(Default, Clone)]
struct PerPredicateIndex {
    facts: Vec<(Statement, PodRef)>,

    arg_indexes: Vec<std::collections::BTreeMap<IndexKey, Vec<usize>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PredicateKey {
    Native(pod2::middleware::NativePredicate),
    Custom(CprKey),
}

impl PartialOrd for PredicateKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PredicateKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (PredicateKey::Native(n1), PredicateKey::Native(n2)) => {
                // Cast the enums to their integer representation for comparison.
                (*n1 as isize).cmp(&(*n2 as isize))
            }
            (PredicateKey::Custom(c1), PredicateKey::Custom(c2)) => c1.cmp(c2),
            // Define an arbitrary but consistent order for the different enum variants.
            (PredicateKey::Native(_), PredicateKey::Custom(_)) => std::cmp::Ordering::Less,
            (PredicateKey::Custom(_), PredicateKey::Native(_)) => std::cmp::Ordering::Greater,
        }
    }
}

/// Immutable, deterministically ordered EDB built from pods and/or signed dictionaries.
#[derive(Default, Clone)]
pub struct ImmutableEdb {
    per_predicate_indexes: std::collections::BTreeMap<PredicateKey, PerPredicateIndex>,

    // Copied Contains facts: (root, key_hash) -> Vec<(value, PodRef)>
    contains_copied: std::collections::BTreeMap<(Hash, Hash), Vec<(Value, PodRef)>>,
    // Full dictionaries registered: root -> key_hash -> value
    full_dicts: std::collections::BTreeMap<Hash, std::collections::BTreeMap<Hash, Value>>,
    // Original full dictionary objects by root (used for replay)
    full_dict_objs: std::collections::BTreeMap<Hash, Dictionary>,
    signed_dicts: std::collections::BTreeMap<Hash, SignedDict>,
    // Stored pods by id for replay
    pods: std::collections::BTreeMap<PodRef, MainPod>,
    // Keypairs registered: public key -> secret key
    keypairs: std::collections::BTreeMap<OrderedPublicKey, SecretKey>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OrderedPublicKey(PublicKey);

impl std::cmp::Ord for OrderedPublicKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.to_string().cmp(&other.0.to_string())
    }
}

impl std::cmp::PartialOrd for OrderedPublicKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Ordered key for indexing CustomPredicateRef by (batch_id, index)
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CprKey {
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

#[derive(Default)]
pub struct ImmutableEdbBuilder {
    inner: ImmutableEdb,
}

impl ImmutableEdbBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn add_statement_for_test(mut self, statement: Statement, pod_ref: PodRef) -> Self {
        self.add_statement(statement, pod_ref);
        self
    }

    fn add_statement(&mut self, statement: Statement, pod_ref: PodRef) {
        use pod2::middleware::{Key, Statement, StatementArg};
        if let Statement::Contains(
            ValueRef::Literal(root_val),
            ValueRef::Literal(key_val),
            ValueRef::Literal(val),
        ) = &statement
        {
            if let Ok(key_str) = String::try_from(key_val.typed()) {
                let root = Hash::from(root_val.raw());
                let key = Key::from(key_str);
                self.inner
                    .contains_copied
                    .entry((root, key.hash()))
                    .or_default()
                    .push((val.clone(), pod_ref.clone()));
            }
        }

        let pred_key = if let Some(native_pred) = native_predicate_from_statement(&statement) {
            PredicateKey::Native(native_pred)
        } else if let Statement::Custom(cpr, _) = &statement {
            PredicateKey::Custom(CprKey::from(cpr))
        } else {
            // Contains is handled above, so this should not be reached for other statement types.
            return;
        };

        let index = self
            .inner
            .per_predicate_indexes
            .entry(pred_key)
            .or_default();

        let fact_id = index.facts.len();
        let args = statement.args();
        index.facts.push((statement.clone(), pod_ref));

        let arity = args.len();
        while index.arg_indexes.len() < arity {
            index.arg_indexes.push(std::collections::BTreeMap::new());
        }
        for (i, arg) in args.iter().enumerate() {
            let keys = match arg {
                StatementArg::Literal(v) => vec![
                    IndexKey::Literal(RawOrdValue(v.clone())),
                    IndexKey::AnyLiteral,
                ],
                StatementArg::Key(ak) => vec![
                    IndexKey::FullAnchoredKey(ak.root, ak.key.hash()),
                    IndexKey::PartialAnchoredKey(ak.key.hash()),
                ],
                StatementArg::None => vec![],
            };
            for key in keys {
                index.arg_indexes[i].entry(key).or_default().push(fact_id);
            }
        }
    }

    pub fn add_full_kv(mut self, root: Hash, key: Key, val: Value) -> Self {
        self.inner
            .full_dicts
            .entry(root)
            .or_default()
            .insert(key.hash(), val);
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

    pub fn build(self) -> ImmutableEdb {
        self.inner
    }

    /// Ingest a MainPod: store it and index its public statements and dictionaries.
    pub fn add_main_pod(mut self, pod: &MainPod) -> Self {
        let pod_ref = PodRef(pod.id());
        self.inner.pods.insert(pod_ref.clone(), pod.clone());
        for st in pod.public_statements.iter() {
            self.add_statement(st.clone(), pod_ref.clone());

            for arg in st.args() {
                if let pod2::middleware::StatementArg::Literal(v) = arg {
                    if let pod2::middleware::TypedValue::Dictionary(dict) = v.typed() {
                        self = self.add_full_dict(dict.clone());
                    }
                }
            }
        }
        self
    }

    pub fn add_keypair(mut self, public_key: PublicKey, secret_key: SecretKey) -> Self {
        self.inner
            .keypairs
            .insert(OrderedPublicKey(public_key), secret_key);
        self
    }
}

fn native_predicate_from_statement(
    statement: &Statement,
) -> Option<pod2::middleware::NativePredicate> {
    match statement.predicate() {
        pod2::middleware::Predicate::Native(np) => Some(np),
        _ => None,
    }
}

impl ImmutableEdb {
    fn query(&self, pred: PredicateKey, args: &[ArgSel]) -> Vec<(Statement, PodRef)> {
        // 1. Get the index for the predicate.
        let index = match self.per_predicate_indexes.get(&pred) {
            Some(idx) => idx,
            None => return vec![], // No facts for this predicate.
        };

        // 2. Collect candidate sets for all arguments from the indexes.
        let mut candidate_sets: Vec<std::collections::HashSet<usize>> = Vec::new();

        for (i, arg_sel) in args.iter().enumerate() {
            if i >= index.arg_indexes.len() {
                // This case can happen if a statement has variable arity (e.g., Custom)
                // and we're querying with more arguments than some facts have.
                return vec![];
            }

            let index_keys = match arg_sel {
                ArgSel::Literal(v) => vec![IndexKey::Literal(RawOrdValue((*v).clone()))],
                ArgSel::Val => vec![IndexKey::AnyLiteral],
                ArgSel::AkExact { root, key } => {
                    vec![IndexKey::FullAnchoredKey(**root, key.hash())]
                }
                ArgSel::AkByKey(_) => {
                    // This is tricky. We'd need to scan all FullAnchoredKey entries.
                    // For now, let's just not use the index for this case.
                    // This will result in a full scan for queries that use it.
                    vec![]
                }
            };

            if index_keys.is_empty() {
                candidate_sets.push((0..index.facts.len()).collect());
                continue;
            }

            let mut arg_candidates = std::collections::HashSet::new();
            for index_key in index_keys {
                if let Some(candidates) = index.arg_indexes[i].get(&index_key) {
                    arg_candidates.extend(candidates.iter().copied());
                }
            }
            if !arg_candidates.is_empty() {
                candidate_sets.push(arg_candidates);
            } else {
                return vec![];
            }
        }

        // 3. Intersect all candidate sets to find the final list of fact IDs.
        let final_candidates = if let Some(first_set) = candidate_sets.first() {
            let mut intersection = first_set.clone();
            for other_set in candidate_sets.iter().skip(1) {
                intersection.retain(|item| other_set.contains(item));
            }
            intersection.into_iter().collect::<Vec<_>>()
        } else {
            // No indexed arguments were provided; this would be a full scan.
            // For now, we return empty, but this could be changed to scan all facts if needed.
            (0..index.facts.len()).collect()
        };

        // 4. Retrieve and filter the final facts.
        let mut results = Vec::new();
        for id in final_candidates {
            if let Some((stmt, pod_ref)) = index.facts.get(id) {
                let stmt_args = stmt.args();
                let mut all_match = true;
                for (i, arg_sel) in args.iter().enumerate() {
                    if i < stmt_args.len() {
                        if !matches_arg_sel(&stmt_args[i], arg_sel) {
                            all_match = false;
                            break;
                        }
                    } else {
                        all_match = false;
                        break;
                    }
                }
                if all_match {
                    results.push((stmt.clone(), pod_ref.clone()));
                }
            }
        }
        results
    }
}

fn matches_arg_sel(arg: &StatementArg, sel: &ArgSel) -> bool {
    use pod2::middleware::AnchoredKey;
    match sel {
        ArgSel::Literal(v) => matches!(arg, StatementArg::Literal(v0) if v0 == *v),
        ArgSel::Val => matches!(arg, StatementArg::Literal(_)),
        ArgSel::AkByKey(key) => {
            matches!(arg, StatementArg::Key(AnchoredKey { key: k, .. }) if k.hash() == key.hash())
        }
        ArgSel::AkExact { root, key } => {
            matches!(arg, StatementArg::Key(AnchoredKey { root: r, key: k }) if r == *root && k.hash() == key.hash())
        }
    }
}

impl EdbView for ImmutableEdb {
    fn query_binary(&self, pred: BinaryPred, lhs: ArgSel, rhs: ArgSel) -> Vec<(Statement, PodRef)> {
        let native_pred = match pred {
            BinaryPred::Equal => pod2::middleware::NativePredicate::Equal,
            BinaryPred::Lt => pod2::middleware::NativePredicate::Lt,
            BinaryPred::LtEq => pod2::middleware::NativePredicate::LtEq,
            BinaryPred::SignedBy => pod2::middleware::NativePredicate::SignedBy,
        };
        self.query(PredicateKey::Native(native_pred), &[lhs, rhs])
    }

    fn custom_matches(
        &self,
        pred: &CustomPredicateRef,
        filters: &[Option<Value>],
    ) -> Vec<(Vec<Value>, PodRef)> {
        let pred_key = PredicateKey::Custom(CprKey::from(pred));
        let args: Vec<ArgSel> = filters
            .iter()
            .map(|f| match f {
                Some(v) => ArgSel::Literal(v),
                None => ArgSel::Val,
            })
            .collect();

        let results = self.query(pred_key, &args);

        results
            .into_iter()
            .filter_map(|(stmt, pod_ref)| {
                if let Statement::Custom(_, custom_args) = stmt {
                    Some((custom_args, pod_ref))
                } else {
                    None
                }
            })
            .collect()
    }
    fn query_ternary(
        &self,
        pred: TernaryPred,
        a: ArgSel,
        b: ArgSel,
        c: ArgSel,
    ) -> Vec<(Statement, PodRef)> {
        let native_pred = match pred {
            TernaryPred::SumOf => pod2::middleware::NativePredicate::SumOf,
            TernaryPred::ProductOf => pod2::middleware::NativePredicate::ProductOf,
            TernaryPred::MaxOf => pod2::middleware::NativePredicate::MaxOf,
            TernaryPred::HashOf => pod2::middleware::NativePredicate::HashOf,
            TernaryPred::Contains => pod2::middleware::NativePredicate::Contains,
        };
        self.query(PredicateKey::Native(native_pred), &[a, b, c])
    }

    fn contains_value(&self, root: &Hash, key: &Key) -> Option<Value> {
        if let Some(vs) = self.contains_copied.get(&(*root, key.hash())) {
            if let Some((v, _)) = vs.first() {
                return Some(v.clone());
            }
        }
        self.full_dicts
            .get(root)
            .and_then(|m| m.get(&key.hash()).cloned())
    }

    fn contains_source(&self, root: &Hash, key: &Key, val: &Value) -> Option<ContainsSource> {
        if let Some(kvs) = self.full_dicts.get(root) {
            if let Some(v) = kvs.get(&key.hash()) {
                if v == val {
                    return Some(ContainsSource::GeneratedFromFullDict { root: *root });
                }
            }
        }
        if let Some(vs) = self.contains_copied.get(&(*root, key.hash())) {
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
            if *k == key.hash() {
                for (v, pod) in vs.iter() {
                    if v == val {
                        out.push((*root, ContainsSource::Copied { pod: pod.clone() }));
                    }
                }
            }
        }
        for (root, kvs) in self.full_dicts.iter() {
            if let Some(v) = kvs.get(&key.hash()) {
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
            .get(&(*root, key.hash()))
            .cloned()
            .unwrap_or_else(Vec::new)
    }

    fn contains_full_value(&self, root: &Hash, key: &Key) -> Option<Value> {
        self.full_dicts
            .get(root)
            .and_then(|m| m.get(&key.hash()).cloned())
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
        let q = self.query(
            PredicateKey::Native(pod2::middleware::NativePredicate::NotContains),
            &[
                ArgSel::Literal(&Value::from(*root)),
                ArgSel::Literal(&Value::from(key.name())),
            ],
        );
        q.first().map(|(_, pod_ref)| pod_ref.clone())
    }
    fn not_contains_roots_for_key(&self, key: &Key) -> Vec<(Hash, PodRef)> {
        let q = self.query(
            PredicateKey::Native(pod2::middleware::NativePredicate::NotContains),
            &[ArgSel::Val, ArgSel::Literal(&Value::from(key.name()))],
        );
        q.iter()
            .filter_map(|(st, pod_ref)| {
                if let Statement::NotContains(ValueRef::Literal(r), _) = st {
                    Some((Hash::from(r.raw()), pod_ref.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
    fn full_dict_absence(&self, root: &Hash, key: &Key) -> Option<bool> {
        self.full_dicts
            .get(root)
            .map(|map| !map.contains_key(&key.hash()))
    }

    fn resolve_pod(&self, id: &PodRef) -> Option<MainPod> {
        self.pods.get(id).cloned()
    }

    fn custom_any_match(&self, pred: &CustomPredicateRef, filters: &[Option<Value>]) -> bool {
        !self.custom_matches(pred, filters).is_empty()
    }

    fn enumerate_keypairs(&self) -> Vec<(Value, Value)> {
        self.keypairs
            .values()
            .map(|sk| (sk.public_key().into(), sk.clone().into()))
            .collect()
    }

    fn get_secret_key(&self, public_key: &PublicKey) -> Option<&SecretKey> {
        self.keypairs.get(&OrderedPublicKey(*public_key))
    }
}
