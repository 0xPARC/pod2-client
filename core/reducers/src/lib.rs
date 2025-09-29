//! POD2 Reducers: public facade module
//!
//! Re-exports the reducer EDSL and APIs from internal modules.

mod error;
pub use crate::error::{Error, Result};

mod ast;
pub use crate::ast::{Expr, VarKind};

mod builder;
pub use crate::builder::Obj;

mod guards;
pub use crate::guards::{Ctx, Guard};

mod lowering;
pub use crate::lowering::{build_predicate, LoweredPredicate};

mod util;

mod planner;
pub use crate::planner::{attach_signature_to_ops, plan_operations};

mod interpreter;
pub use crate::interpreter::{exec_reducer, exec_reducer_with_aux, ExecAux};

mod macros;

// router module removed; prefer reducer_router! macro

#[cfg(test)]
mod tests;
