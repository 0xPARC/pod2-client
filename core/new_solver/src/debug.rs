use std::fmt::Debug;

use pod2::middleware::CustomPredicateRef;

use crate::CallPattern;

impl Debug for CallPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CallPattern({:?}, {:?})",
            self.pred.predicate().name,
            self.literals
        )
    }
}

pub(crate) struct CustomPredicateRefDebug(pub(crate) CustomPredicateRef);

impl Debug for CustomPredicateRefDebug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CustomPredicateRef({:?})", self.0.predicate().name)
    }
}
