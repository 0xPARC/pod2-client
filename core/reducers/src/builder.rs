use std::rc::Rc;

use pod2::middleware::Value;

use crate::ast::{Expr, VarKind};

/// Key argument that can be either a static string or a dynamic expression.
pub enum KeyArg {
    Static(String),
    Dynamic(Rc<Expr>),
}

impl From<&str> for KeyArg {
    fn from(s: &str) -> Self {
        KeyArg::Static(s.to_string())
    }
}
impl From<String> for KeyArg {
    fn from(s: String) -> Self {
        KeyArg::Static(s)
    }
}
impl From<Obj> for KeyArg {
    fn from(o: Obj) -> Self {
        KeyArg::Dynamic(o.0)
    }
}
impl<'a> From<&'a Obj> for KeyArg {
    fn from(o: &'a Obj) -> Self {
        KeyArg::Dynamic(o.0.clone())
    }
}

/// Thin ergonomic wrapper around `Expr` used by the builder API.
#[derive(Clone, Debug)]
pub struct Obj(Rc<Expr>);

impl Obj {
    pub fn var(kind: VarKind) -> Self {
        Obj(Rc::new(Expr::Var(kind)))
    }
    /// Set a field on a dictionary, returning a new dictionary root.
    /// Repeated calls chain without needing an explicit build().
    pub fn set(&self, key: impl Into<KeyArg>, val: impl Into<Obj>) -> Self {
        let key = key.into();
        let val = val.into();
        match key {
            KeyArg::Static(k) => match self.expr() {
                crate::ast::Expr::DictUpdateChain { base, sets } => {
                    let mut new_sets = sets.clone();
                    new_sets.push((k, val.0));
                    Obj(Rc::new(crate::ast::Expr::DictUpdateChain {
                        base: base.clone(),
                        sets: new_sets,
                    }))
                }
                _ => Obj(Rc::new(crate::ast::Expr::DictUpdateChain {
                    base: self.0.clone(),
                    sets: vec![(k, val.0)],
                })),
            },
            KeyArg::Dynamic(kexpr) => Obj(Rc::new(crate::ast::Expr::ContainerUpdate {
                dict: self.0.clone(),
                key: kexpr,
                val: val.0,
            })),
        }
    }
    /// A fresh empty dictionary literal.
    pub fn empty_dict() -> Self {
        Obj(Rc::new(Expr::EmptyDict))
    }
    /// A fresh empty set literal.
    pub fn empty_set() -> Self {
        Obj(Rc::new(Expr::EmptySet))
    }
    pub fn expr(&self) -> &crate::ast::Expr {
        self.0.as_ref()
    }
    pub fn const_val(v: impl Into<Value>) -> Self {
        Obj(Rc::new(Expr::Const(v.into())))
    }
    pub fn get(&self, key: impl Into<KeyArg>) -> Self {
        match key.into() {
            KeyArg::Static(k) => Obj(Rc::new(Expr::Field(self.0.clone(), k))),
            KeyArg::Dynamic(kexpr) => Obj(Rc::new(Expr::ContainerLookup {
                dict: self.0.clone(),
                key: kexpr,
            })),
        }
    }
    pub fn add(&self, rhs: impl Into<Obj>) -> Self {
        let rhs_obj = rhs.into();
        Obj(Rc::new(Expr::Add(self.0.clone(), rhs_obj.0)))
    }
    /// Integer multiplication: builds Expr::Mul (→ ProductOf)
    pub fn mul(&self, rhs: impl Into<Obj>) -> Self {
        let rhs_obj = rhs.into();
        Obj(Rc::new(Expr::Mul(self.0.clone(), rhs_obj.0)))
    }
    /// Integer maximum: builds Expr::Max (→ MaxOf)
    pub fn max(&self, rhs: impl Into<Obj>) -> Self {
        let rhs_obj = rhs.into();
        Obj(Rc::new(Expr::Max(self.0.clone(), rhs_obj.0)))
    }
    /// Set operation: returns a new set root as an expression (must be written back via a dict update)
    pub fn insert(&self, elem: impl Into<Obj>) -> Self {
        let elem = elem.into();
        Obj(Rc::new(Expr::SetInsert {
            set: self.0.clone(),
            elem: elem.0,
        }))
    }
    pub fn update(&self) -> UpdateBuilder {
        UpdateBuilder {
            base: self.0.clone(),
            sets: Vec::new(),
        }
    }
    /// Set operation: deletion (returns new set root)
    pub fn delete(&self, elem: impl Into<Obj>) -> Self {
        let elem = elem.into();
        Obj(Rc::new(Expr::SetDelete {
            set: self.0.clone(),
            elem: elem.0,
        }))
    }
    /// Dict insert (returns new dict root)
    pub fn dict_insert(&self, key: impl Into<String>, val: impl Into<Obj>) -> Self {
        let val = val.into();
        Obj(Rc::new(Expr::DictInsert {
            dict: self.0.clone(),
            key: key.into(),
            val: val.0,
        }))
    }
    /// Dict update (returns new dict root)
    pub fn dict_update(&self, key: impl Into<String>, val: impl Into<Obj>) -> Self {
        let val = val.into();
        Obj(Rc::new(Expr::DictUpdate {
            dict: self.0.clone(),
            key: key.into(),
            val: val.0,
        }))
    }
    /// Dict delete (returns new dict root)
    pub fn dict_delete(&self, key: impl Into<String>) -> Self {
        Obj(Rc::new(Expr::DictDelete {
            dict: self.0.clone(),
            key: key.into(),
        }))
    }
    /// Dynamic dict insert with expression key (returns new dict root)
    pub fn dict_insert_expr(&self, key: impl Into<Obj>, val: impl Into<Obj>) -> Self {
        let key = key.into();
        let val = val.into();
        Obj(Rc::new(Expr::ContainerInsert {
            dict: self.0.clone(),
            key: key.0,
            val: val.0,
        }))
    }
    /// Dynamic dict update with expression key (returns new dict root)
    pub fn dict_update_expr(&self, key: impl Into<Obj>, val: impl Into<Obj>) -> Self {
        let key = key.into();
        let val = val.into();
        Obj(Rc::new(Expr::ContainerUpdate {
            dict: self.0.clone(),
            key: key.0,
            val: val.0,
        }))
    }
    /// Dynamic dict delete with expression key (returns new dict root)
    pub fn dict_delete_expr(&self, key: impl Into<Obj>) -> Self {
        let key = key.into();
        Obj(Rc::new(Expr::ContainerDelete {
            dict: self.0.clone(),
            key: key.0,
        }))
    }

    // Ergonomic aliases: at-style container edits
    pub fn insert_at(&self, key: impl Into<Obj>, val: impl Into<Obj>) -> Self {
        self.dict_insert_expr(key, val)
    }
    pub fn update_at(&self, key: impl Into<Obj>, val: impl Into<Obj>) -> Self {
        self.dict_update_expr(key, val)
    }
    pub fn delete_at(&self, key: impl Into<Obj>) -> Self {
        self.dict_delete_expr(key)
    }
    /// Unified remove for dictionary entries: accepts static or dynamic key
    pub fn remove(&self, key: impl Into<KeyArg>) -> Self {
        match key.into() {
            KeyArg::Static(k) => Obj(Rc::new(Expr::DictDelete {
                dict: self.0.clone(),
                key: k,
            })),
            KeyArg::Dynamic(kexpr) => Obj(Rc::new(Expr::ContainerDelete {
                dict: self.0.clone(),
                key: kexpr,
            })),
        }
    }
    /// Unified put (strict insert) for dictionary entries: accepts static or dynamic key
    pub fn put(&self, key: impl Into<KeyArg>, val: impl Into<Obj>) -> Self {
        let val = val.into();
        match key.into() {
            KeyArg::Static(k) => Obj(Rc::new(Expr::DictInsert {
                dict: self.0.clone(),
                key: k,
                val: val.0,
            })),
            KeyArg::Dynamic(kexpr) => Obj(Rc::new(Expr::ContainerInsert {
                dict: self.0.clone(),
                key: kexpr,
                val: val.0,
            })),
        }
    }
    /// Array update (returns new array root)
    pub fn array_update(&self, index: impl Into<Obj>, val: impl Into<Obj>) -> Self {
        let idx = index.into();
        let val = val.into();
        Obj(Rc::new(Expr::ArrayUpdate {
            array: self.0.clone(),
            index: idx.0,
            val: val.0,
        }))
    }
}

impl From<Expr> for Obj {
    fn from(e: Expr) -> Self {
        Obj(Rc::new(e))
    }
}

// Convenience: allow passing plain literals (i64, bool, String, etc.)
// anywhere an `Obj` is expected. This makes calls like `.add(10)` work.
impl<T> From<T> for Obj
where
    T: Into<Value>,
{
    fn from(t: T) -> Self {
        Obj::const_val(t)
    }
}

impl From<&Obj> for Obj {
    fn from(o: &Obj) -> Self {
        o.clone()
    }
}

impl<T> std::ops::Add<T> for Obj
where
    T: Into<Obj>,
{
    type Output = Obj;
    fn add(self, rhs: T) -> Self::Output {
        let r: Obj = rhs.into();
        Obj(Rc::new(Expr::Add(self.0, r.0)))
    }
}

impl<T> std::ops::Mul<T> for Obj
where
    T: Into<Obj>,
{
    type Output = Obj;
    fn mul(self, rhs: T) -> Self::Output {
        let r: Obj = rhs.into();
        Obj(Rc::new(Expr::Mul(self.0, r.0)))
    }
}

/// Builder for immutable dictionary updates; corresponds to a chain of Podlang
/// `DictUpdate(out, dict, key, val)` statements.
#[derive(Clone, Debug)]
pub struct UpdateBuilder {
    pub(crate) base: Rc<Expr>,
    pub(crate) sets: Vec<(String, Rc<Expr>)>,
}

impl UpdateBuilder {
    pub fn set(mut self, key: impl Into<String>, val: impl Into<Obj>) -> Self {
        let key = key.into();
        let val = val.into();
        self.sets.push((key, val.0));
        self
    }
    pub fn build(self) -> Obj {
        Obj(Rc::new(Expr::DictUpdateChain {
            base: self.base,
            sets: self.sets,
        }))
    }
}
