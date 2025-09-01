use std::collections::{HashMap, HashSet};

use pod2::middleware::{CustomPredicateRef, Hash, Key, Statement, StatementTmplArg, Value};

/// Unique identifier for a frame.
pub type FrameId = usize;

/// Unique key identifying a subgoal (predicate + ground args).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CallKey {
    pub pred_name: String,
    pub ground_args: Vec<Value>,
}

/// Minimal subgoal table stub (for future tabling/custom predicates).
#[derive(Default, Debug)]
pub struct SubgoalTable {
    pub answers: Vec<(Statement, OpTag)>,
    pub active_producers: usize,
    pub is_complete: bool,
}

/// OpTag captures how a statement/premise was obtained.
#[derive(Clone, Debug, PartialEq)]
pub enum OpTag {
    CopyStatement {
        source: PodRef, // The PodRef of the source Pod we copied the statement from
    },
    FromLiterals,
    Derived {
        premises: Vec<(Statement, OpTag)>,
    },
    CustomDeduction {
        rule_id: CustomPredicateRef,
        premises: Vec<(Statement, OpTag)>,
    },
    /// A Contains premise that is justified because the solver has a full dictionary
    /// and can generate the membership fact (proof attached later at compilation time).
    GeneratedContains {
        root: Hash, // The Merkle root of the dictionary
        key: Key,
        value: Value,
    },
}

/// Provenance reference to a POD for CopyStatement.
/// This is the Pod::statements_hash.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PodRef(pub Hash);

/// Local constraint store per producer branch.
#[derive(Clone, Debug, Default)]
pub struct ConstraintStore {
    pub bindings: HashMap<usize, Value>,
    pub residual_constraints: Vec<StatementTmplArg>,
    pub premises: Vec<(Statement, OpTag)>,
    pub input_pods: HashSet<PodRef>,
    pub operation_count: usize,
    /// Accumulated lower bound on operations for pending subcalls (structural),
    /// carried along recursive descent to enable early pruning before realization.
    pub accumulated_lb_ops: usize,
    /// Stack of pending custom deductions to materialize upon success.
    pub pending_custom: Vec<PendingCustom>,
}

#[derive(Clone, Debug)]
pub struct PendingCustom {
    pub rule_id: CustomPredicateRef,
    /// Head arguments expressed as template args using the remapped wildcards.
    pub head_args: Vec<StatementTmplArg>,
    /// Number of premises present in the store when this pending head was registered.
    /// Premises added after this point are considered the body premises for this head.
    pub base_premises_len: usize,
    /// Remapped body templates for this custom rule expansion, ordered as per template.
    pub templates: Vec<pod2::middleware::StatementTmpl>,
    /// Ordered body slots aligned to `templates`; None means not yet satisfied (or OR-branch not taken).
    pub slots: Vec<Option<(Statement, super::OpTag)>>,
}

/// A wrapper for `Value` that orders by its raw bytes commitment for use in BTree* maps.
#[derive(Clone, Debug)]
pub struct RawOrdValue(pub Value);

impl std::cmp::PartialEq for RawOrdValue {
    fn eq(&self, other: &Self) -> bool {
        self.0.raw() == other.0.raw()
    }
}
impl std::cmp::Eq for RawOrdValue {}
impl std::cmp::PartialOrd for RawOrdValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl std::cmp::Ord for RawOrdValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.raw().cmp(&other.0.raw())
    }
}
