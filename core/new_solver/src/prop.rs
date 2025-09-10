use pod2::middleware::StatementTmplArg;

use crate::types::OpTag;

/// Outcome of attempting to satisfy a goal via an OpHandler.
#[derive(Clone, Debug)]
pub enum PropagatorResult {
    Entailed {
        bindings: Vec<(usize, pod2::middleware::Value)>,
        op_tag: OpTag,
    },
    Suspend {
        on: Vec<usize>,
    },
    Choices {
        alternatives: Vec<Choice>,
    },
    Contradiction,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub bindings: Vec<(usize, pod2::middleware::Value)>,
    pub op_tag: OpTag,
}

/// Utility: determine wildcard ids referenced by a template arg (minimal for MVP).
pub fn wildcards_in_args(args: &[StatementTmplArg]) -> Vec<usize> {
    args.iter()
        .filter_map(|a| match a {
            StatementTmplArg::Wildcard(w) => Some(w.index),
            StatementTmplArg::AnchoredKey(w, _key) => Some(w.index),
            _ => None,
        })
        .collect()
}
