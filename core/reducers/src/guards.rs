use pod2::middleware::NativePredicate;

use crate::{ast::VarKind, builder::Obj};

/// Top-level reducer constraints (guards) that must hold alongside the patch.
#[derive(Clone, Debug)]
pub enum Guard {
    /// Generic native predicate with arguments (Obj expressions).
    Native(NativePredicate, Vec<Obj>),
}

/// Construction context exposed to reducer author functions.
pub struct Ctx {
    pub(crate) guards: Vec<Guard>,
    next_witness: usize,
}

impl Ctx {
    pub fn new() -> Self {
        Self {
            guards: Vec::new(),
            next_witness: 0,
        }
    }
    /// A fresh empty dictionary literal.
    pub fn empty_dict(&self) -> Obj {
        Obj::empty_dict()
    }
    /// A fresh empty set literal.
    pub fn empty_set(&self) -> Obj {
        Obj::empty_set()
    }
    pub fn state(&self) -> Obj {
        Obj::var(VarKind::Prev)
    }
    pub fn action(&self) -> Obj {
        Obj::var(VarKind::Action)
    }
    /// Expose guards collected so far (for macro/internal use and external callers).
    pub fn guards(&self) -> &[Guard] {
        &self.guards
    }
    /// Generic require with explicit predicate and n-args helper overloads.
    pub fn require_native(&mut self, pred: NativePredicate, args: Vec<Obj>) {
        self.guards.push(Guard::Native(pred, args));
    }
    pub fn require2(&mut self, pred: NativePredicate, a1: impl Into<Obj>, a2: impl Into<Obj>) {
        self.require_native(pred, vec![a1.into(), a2.into()]);
    }
    pub fn require3(
        &mut self,
        pred: NativePredicate,
        a1: impl Into<Obj>,
        a2: impl Into<Obj>,
        a3: impl Into<Obj>,
    ) {
        self.require_native(pred, vec![a1.into(), a2.into(), a3.into()]);
    }
    pub fn require4(
        &mut self,
        pred: NativePredicate,
        a1: impl Into<Obj>,
        a2: impl Into<Obj>,
        a3: impl Into<Obj>,
        a4: impl Into<Obj>,
    ) {
        self.require_native(pred, vec![a1.into(), a2.into(), a3.into(), a4.into()]);
    }
}

// Macro helpers to generate predicate-specific ergonomic require_* methods with arity checks
macro_rules! require_impl_oa {
    ($fn_name:ident, $pred_name:ident, 2) => {
        pub fn $fn_name(&mut self, a1: impl Into<Obj>, a2: impl Into<Obj>) {
            self.require2(NativePredicate::$pred_name, a1, a2)
        }
    };
    ($fn_name:ident, $pred_name:ident, 3) => {
        pub fn $fn_name(&mut self, a1: impl Into<Obj>, a2: impl Into<Obj>, a3: impl Into<Obj>) {
            self.require3(NativePredicate::$pred_name, a1, a2, a3)
        }
    };
    ($fn_name:ident, $pred_name:ident, 4) => {
        pub fn $fn_name(
            &mut self,
            a1: impl Into<Obj>,
            a2: impl Into<Obj>,
            a3: impl Into<Obj>,
            a4: impl Into<Obj>,
        ) {
            self.require4(NativePredicate::$pred_name, a1, a2, a3, a4)
        }
    };
}

impl Ctx {
    // Common 2-arg comparisons
    require_impl_oa!(require_eq, Equal, 2);
    require_impl_oa!(require_ne, NotEqual, 2);
    require_impl_oa!(require_lt, Lt, 2);
    require_impl_oa!(require_lte, LtEq, 2);
    require_impl_oa!(require_gt, Gt, 2);
    require_impl_oa!(require_gte, GtEq, 2);
    // Membership/containment
    require_impl_oa!(require_contains, Contains, 3);
    require_impl_oa!(require_not_contains, NotContains, 2);
    require_impl_oa!(require_dict_contains, DictContains, 3);
    require_impl_oa!(require_dict_not_contains, DictNotContains, 2);
    require_impl_oa!(require_set_contains, SetContains, 2);
    require_impl_oa!(require_set_not_contains, SetNotContains, 2);
    require_impl_oa!(require_array_contains, ArrayContains, 3);
    // Arithmetic / hashing
    require_impl_oa!(require_sum_of, SumOf, 3);
    require_impl_oa!(require_product_of, ProductOf, 3);
    require_impl_oa!(require_max_of, MaxOf, 3);
    require_impl_oa!(require_hash_of, HashOf, 3);
    // Keys and signatures
    require_impl_oa!(require_public_key_of, PublicKeyOf, 2);
    require_impl_oa!(require_signed_by, SignedBy, 2);
    // Container mutations (sugar; frontend will desugar to Container*)
    require_impl_oa!(require_dict_insert, DictInsert, 4);
    require_impl_oa!(require_dict_update, DictUpdate, 4);
    require_impl_oa!(require_dict_delete, DictDelete, 3);
    require_impl_oa!(require_set_insert, SetInsert, 3);
    require_impl_oa!(require_set_delete, SetDelete, 3);
    require_impl_oa!(require_array_update, ArrayUpdate, 4);

    /// Lookup a value in a dictionary by a (possibly dynamic) key and return a
    /// symbolic witness bound by a DictContains guard.
    pub fn lookup(&mut self, dict: Obj, key: Obj) -> Obj {
        self.next_witness += 1;
        let name = format!("lk_{}", self.next_witness);
        let w = Obj::var(VarKind::Witness(name));
        self.require_dict_contains(dict, key, w.clone());
        w
    }

    // Custom predicate calls removed in simplified API
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new()
    }
}
