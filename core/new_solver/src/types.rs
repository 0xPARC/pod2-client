use std::collections::{HashMap, HashSet};

use pod2::middleware::{
    CustomPredicateRef, Hash as RootHash, Key, Statement, StatementTmplArg, Value,
};

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
        source: PodRef,
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
        root: RootHash,
        key: Key,
        value: Value,
    },
}

/// Provenance reference to a POD for CopyStatement; opaque at this layer.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PodRef(pub RootHash);

/// Local constraint store per producer branch.
#[derive(Clone, Debug, Default)]
pub struct ConstraintStore {
    pub bindings: HashMap<usize, Value>,
    pub residual_constraints: Vec<StatementTmplArg>,
    pub premises: Vec<(Statement, OpTag)>,
    pub input_pods: HashSet<PodRef>,
    pub operation_count: usize,
}
