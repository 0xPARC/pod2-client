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

mod batch;
pub use crate::batch::BatchBuilder;

mod router;
pub use crate::router::{Router, RouterBuilder};

mod bottom_up;
pub use crate::bottom_up::{
    apply_child_receipts, forum_apply_post_receipts, forum_apply_posts_change, posts_apply_change,
    ChildEffect, LeafReceipt,
};

#[cfg(test)]
mod tests;
