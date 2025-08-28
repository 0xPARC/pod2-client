use std::collections::HashMap;

use pod2::middleware::{NativePredicate, StatementTmplArg};

use crate::{edb::EdbView, prop::PropagatorResult, types::ConstraintStore};

/// One concrete way to satisfy a native goal of a given predicate.
pub trait OpHandler: Send + Sync {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult;
}

#[derive(Default)]
pub struct OpRegistry {
    table: HashMap<NativePredicate, Vec<Box<dyn OpHandler>>>,
}

impl OpRegistry {
    pub fn register(&mut self, p: NativePredicate, h: Box<dyn OpHandler>) {
        self.table.entry(p).or_default().push(h);
    }
    pub fn get(&self, p: NativePredicate) -> &[Box<dyn OpHandler>] {
        self.table.get(&p).map(|v| &v[..]).unwrap_or(&[])
    }
}
