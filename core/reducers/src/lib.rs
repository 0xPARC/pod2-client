//! POD2 Reducers: public facade module
//!
//! Re-exports the reducer EDSL and APIs from internal modules.

mod error;
pub use crate::error::{Error, Result};

mod ast;
pub use crate::ast::{Expr, VarKind};

mod builder;
pub use crate::builder::{Obj, UpdateBuilder};

mod guards;
pub use crate::guards::{Ctx, Guard};

mod lowering;
pub use crate::lowering::{build_predicate, LoweredPredicate};

mod util;

mod planner;
pub use crate::planner::plan_operations;

mod interpreter;
pub use crate::interpreter::exec_reducer;

mod macros;

mod router;
pub use crate::router::{Router, RouterBuilder};

#[cfg(test)]
mod tests;
