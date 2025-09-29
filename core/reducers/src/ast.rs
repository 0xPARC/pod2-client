use std::rc::Rc;

use pod2::middleware::Value;

/// Symbolic variables that correspond to Podlang predicate wildcards.
/// - `Prev`: previous state Dictionary root
/// - `Action`: action Dictionary root
/// - `Next`: next state Dictionary root (the reducer's result)
/// - `Witness(String)`: private existential wildcard created during lowering
#[derive(Clone, Debug)]
pub enum VarKind {
    Prev,
    Action,
    Next,
    Witness(String),
}

/// Author-intent AST nodes (symbolic only; not executed here).
#[derive(Clone, Debug)]
pub enum Expr {
    Var(VarKind),
    Const(Value),
    Field(Rc<Expr>, String),
    /// Construct a fresh empty dictionary (using default Params)
    EmptyDict,
    /// Construct a fresh empty set (using default Params)
    EmptySet,
    Add(Rc<Expr>, Rc<Expr>),
    /// Integer multiplication: lowers to ProductOf(out, a, b)
    Mul(Rc<Expr>, Rc<Expr>),
    /// Integer maximum: lowers to MaxOf(out, a, b)
    Max(Rc<Expr>, Rc<Expr>),
    SetInsert {
        set: Rc<Expr>,
        elem: Rc<Expr>,
    },
    SetDelete {
        set: Rc<Expr>,
        elem: Rc<Expr>,
    },
    DictInsert {
        dict: Rc<Expr>,
        key: String,
        val: Rc<Expr>,
    },
    DictUpdate {
        dict: Rc<Expr>,
        key: String,
        val: Rc<Expr>,
    },
    DictDelete {
        dict: Rc<Expr>,
        key: String,
    },
    ContainerLookup {
        dict: Rc<Expr>,
        key: Rc<Expr>,
    },
    ContainerInsert {
        dict: Rc<Expr>,
        key: Rc<Expr>,
        val: Rc<Expr>,
    },
    ContainerUpdate {
        dict: Rc<Expr>,
        key: Rc<Expr>,
        val: Rc<Expr>,
    },
    ContainerDelete {
        dict: Rc<Expr>,
        key: Rc<Expr>,
    },
    // Helper to build multiple successive DictUpdate operations ergonomically
    DictUpdateChain {
        base: Rc<Expr>,
        sets: Vec<(String, Rc<Expr>)>,
    },
    ArrayUpdate {
        array: Rc<Expr>,
        index: Rc<Expr>,
        val: Rc<Expr>,
    },
}
