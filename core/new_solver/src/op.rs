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

pub struct OpRegistry {
    table: HashMap<NativePredicate, Vec<Box<dyn OpHandler>>>,
}

impl Default for OpRegistry {
    fn default() -> Self {
        let mut reg = OpRegistry {
            table: HashMap::new(),
        };
        crate::handlers::register_equal_handlers(&mut reg);
        crate::handlers::register_lt_handlers(&mut reg);
        crate::handlers::register_sumof_handlers(&mut reg);
        crate::handlers::register_signed_by_handlers(&mut reg);
        crate::handlers::register_contains_handlers(&mut reg);
        crate::handlers::register_not_contains_handlers(&mut reg);
        crate::handlers::register_container_insert_handlers(&mut reg);
        crate::handlers::register_container_update_handlers(&mut reg);
        crate::handlers::register_productof_handlers(&mut reg);
        crate::handlers::register_maxof_handlers(&mut reg);
        crate::handlers::register_hashof_handlers(&mut reg);
        crate::handlers::register_not_equal_handlers(&mut reg);
        reg
    }
}

impl OpRegistry {
    pub fn register(&mut self, p: NativePredicate, h: Box<dyn OpHandler>) {
        self.table.entry(p).or_default().push(h);
    }
    pub fn get(&self, p: NativePredicate) -> &[Box<dyn OpHandler>] {
        self.table.get(&p).map(|v| &v[..]).unwrap_or(&[])
    }
}
