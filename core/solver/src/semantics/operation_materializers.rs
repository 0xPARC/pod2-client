//! Operation-centric materialization for native Datalog predicates.
//!
//! This module replaces the previous PredicateHandler system with direct
//! OperationMaterializer dispatch, making the Operation ↔ Solver relationship explicit.

use log::trace;
use pod2::{
    frontend::Operation,
    middleware::{
        hash_values, Key, NativeOperation, NativePredicate, OperationType, Statement, TypedValue,
        Value, ValueRef, SELF,
    },
};

use crate::{
    db::FactDB,
    engine::semi_naive::{Fact, FactSource, Relation},
    error::SolverError,
};

/// An enum that dispatches to the correct materializer for a given native operation.
/// Each variant corresponds directly to a NativeOperation type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationMaterializer {
    // Core native operations. This mirrors NativeOperation, but allows us to defer
    // supporting new operations until we have working implementations.
    None,
    NewEntry,
    CopyStatement,
    EqualFromEntries,
    NotEqualFromEntries,
    LtEqFromEntries,
    LtFromEntries,
    TransitiveEqualFromStatements,
    LtToNotEqual,
    ContainsFromEntries,
    NotContainsFromEntries,
    SumOf,
    ProductOf,
    MaxOf,
    HashOf,
    PublicKeyOf,
}

impl OperationMaterializer {
    pub fn operation_type(&self) -> OperationType {
        let native_op = match self {
            Self::None => NativeOperation::None,
            Self::NewEntry => NativeOperation::NewEntry,
            Self::CopyStatement => NativeOperation::CopyStatement,
            Self::EqualFromEntries => NativeOperation::EqualFromEntries,
            Self::NotEqualFromEntries => NativeOperation::NotEqualFromEntries,
            Self::LtEqFromEntries => NativeOperation::LtEqFromEntries,
            Self::LtFromEntries => NativeOperation::LtFromEntries,
            Self::TransitiveEqualFromStatements => NativeOperation::TransitiveEqualFromStatements,
            Self::LtToNotEqual => NativeOperation::LtToNotEqual,
            Self::ContainsFromEntries => NativeOperation::ContainsFromEntries,
            Self::NotContainsFromEntries => NativeOperation::NotContainsFromEntries,
            Self::SumOf => NativeOperation::SumOf,
            Self::ProductOf => NativeOperation::ProductOf,
            Self::MaxOf => NativeOperation::MaxOf,
            Self::HashOf => NativeOperation::HashOf,
            Self::PublicKeyOf => NativeOperation::PublicKeyOf,
        };
        OperationType::Native(native_op)
    }

    /// Returns all materializers that can produce statements for the given predicate type
    pub fn materializers_for_predicate(pred: NativePredicate) -> &'static [OperationMaterializer] {
        match pred {
            NativePredicate::Equal => &[
                Self::EqualFromEntries,
                Self::TransitiveEqualFromStatements,
                Self::NewEntry,
                Self::CopyStatement,
            ],
            NativePredicate::NotEqual => &[
                Self::NotEqualFromEntries,
                Self::LtToNotEqual,
                Self::CopyStatement,
            ],
            NativePredicate::Lt => &[Self::LtFromEntries, Self::CopyStatement],
            NativePredicate::LtEq => &[Self::LtEqFromEntries, Self::CopyStatement],
            NativePredicate::Contains => &[Self::ContainsFromEntries, Self::CopyStatement],
            NativePredicate::NotContains => &[Self::NotContainsFromEntries, Self::CopyStatement],
            NativePredicate::SumOf => &[Self::SumOf, Self::CopyStatement],
            NativePredicate::ProductOf => &[Self::ProductOf, Self::CopyStatement],
            NativePredicate::MaxOf => &[Self::MaxOf, Self::CopyStatement],
            NativePredicate::HashOf => &[Self::HashOf, Self::CopyStatement],
            NativePredicate::PublicKeyOf => &[Self::PublicKeyOf],
            NativePredicate::None => &[Self::None],
            NativePredicate::False => &[], // No operations can produce False
            // Syntactic sugar predicates are transformed by frontend compiler, so we don't need materializers for them.
            NativePredicate::DictContains => &[],
            NativePredicate::DictNotContains => &[],
            NativePredicate::SetContains => &[],
            NativePredicate::SetNotContains => &[],
            NativePredicate::ArrayContains => &[],
            NativePredicate::Gt => &[],
            NativePredicate::GtEq => &[],
        }
    }

    /// Attempts to materialize a fact using this operation with the given arguments
    pub fn materialize_relation(
        &self,
        args: &[Option<ValueRef>],
        db: &FactDB,
        predicate: NativePredicate,
    ) -> Relation {
        match self {
            OperationMaterializer::PublicKeyOf => materialize_public_key_of(args, db),
            m => m
                .materialize(args, db, predicate)
                .into_iter()
                .collect::<Relation>(),
        }
    }

    pub fn materialize(
        &self,
        args: &[Option<ValueRef>],
        db: &FactDB,
        predicate: NativePredicate,
    ) -> Option<Fact> {
        match self {
            Self::None => materialize_none(args, db),
            Self::NewEntry => materialize_new_entry(args, db),
            Self::CopyStatement => materialize_copy_statement(args, db, predicate),
            Self::EqualFromEntries => materialize_equal_from_entries(args, db),
            Self::NotEqualFromEntries => materialize_not_equal_from_entries(args, db),
            Self::LtEqFromEntries => materialize_lt_eq_from_entries(args, db),
            Self::LtFromEntries => materialize_lt_from_entries(args, db),
            Self::TransitiveEqualFromStatements => {
                materialize_transitive_equal_from_statements(args, db)
            }
            Self::LtToNotEqual => materialize_lt_to_not_equal(args, db),
            Self::ContainsFromEntries => materialize_contains_from_entries(args, db),
            Self::NotContainsFromEntries => materialize_not_contains_from_entries(args, db),
            Self::SumOf => materialize_sum_of(args, db),
            Self::ProductOf => materialize_product_of(args, db),
            Self::MaxOf => materialize_max_of(args, db),
            Self::HashOf => materialize_hash_of(args, db),
            Self::PublicKeyOf => unimplemented!("PublicKeyOf should use materialize_relation"),
        }
    }

    /// Generates Operation instances for proof construction
    pub fn explain(&self, args: &[ValueRef], db: &FactDB) -> Result<Vec<Operation>, SolverError> {
        match self {
            Self::TransitiveEqualFromStatements => {
                explain_transitive_equal_from_statements(args, db)
            }
            // Most operations have simple explanations that return empty vec for now
            _ => Ok(vec![]),
        }
    }
}

// Individual materializer functions
// Each function contains both precondition checks and materialization logic
//
// Three categories of operations:
// 1. VALUE-BASED COMPUTATIONS: Compare resolved values (e.g., EqualFromEntries, LtFromEntries)
// 2. STATEMENT COPYING: Look up existing statements (CopyStatement)
// 3. STATEMENT DERIVATIONS: Derive new statements from existing ones (e.g., LtToNotEqual, TransitiveEqualFromStatements)

fn materialize_none(_args: &[Option<ValueRef>], _db: &FactDB) -> Option<Fact> {
    // None operation doesn't materialize any facts
    None
}

fn materialize_new_entry(args: &[Option<ValueRef>], _db: &FactDB) -> Option<Fact> {
    // NewEntry operation only applies to Equal statements where first arg is SELF-referencing
    if args.len() != 2 {
        return None;
    }

    let (vr0, vr1) = match (&args[0], &args[1]) {
        (Some(vr0), Some(vr1)) => (vr0, vr1),
        _ => return None, // Both args must be bound
    };

    // Check if first arg is SELF-referencing anchored key
    if let (ValueRef::Key(ak), ValueRef::Literal(_)) = (vr0, vr1) {
        if ak.pod_id == SELF {
            Some(Fact {
                source: FactSource::NewEntry,
                args: vec![vr0.clone(), vr1.clone()],
            })
        } else {
            None
        }
    } else {
        None
    }
}

fn materialize_copy_statement(
    args: &[Option<ValueRef>],
    db: &FactDB,
    predicate: NativePredicate,
) -> Option<Fact> {
    // CopyStatement is the only materializer that can look up existing statements
    // All other materializers should only do value-based computations

    let value_refs: Vec<ValueRef> = args.iter().cloned().collect::<Option<Vec<_>>>()?;

    // Check if a statement already exists for these arguments in the predicate's index
    let statement_exists = match value_refs.len() {
        2 => {
            if let Some(index) = db.get_binary_statement_index(&predicate) {
                index.contains_key(&[value_refs[0].clone(), value_refs[1].clone()])
            } else {
                false
            }
        }
        3 => {
            if let Some(index) = db.get_ternary_statement_index(&predicate) {
                index.contains_key(&[
                    value_refs[0].clone(),
                    value_refs[1].clone(),
                    value_refs[2].clone(),
                ])
            } else {
                false
            }
        }
        _ => false, // Other arities not supported
    };

    if statement_exists {
        Some(Fact {
            source: FactSource::Copy,
            args: value_refs,
        })
    } else {
        None
    }
}

fn materialize_equal_from_entries(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 2 {
        return None;
    }

    // Are all args bound?
    let (vr0, vr1) = match (&args[0], &args[1]) {
        (Some(vr0), Some(vr1)) => (vr0, vr1),
        // Try to deduce with free args
        _ => return materialize_equal_from_entries_with_deduction(args, db),
    };

    // Can all args be resolved to values?
    if let (Some(val0), Some(val1)) = (db.value_ref_to_value(vr0), db.value_ref_to_value(vr1)) {
        // Do direct value comparison
        if val0 == val1 {
            trace!("EqualFromEntries: {val0} == {val1}");
            Some(Fact {
                source: FactSource::Native(NativeOperation::EqualFromEntries),
                args: vec![vr0.clone(), vr1.clone()],
            })
        } else {
            None
        }
    } else {
        // Some arguments are anchored keys we can't resolve
        // CopyStatement will handle looking up existing statements
        None
    }
}

/// Helper function for Equal deduction when some args are unbound
fn materialize_equal_from_entries_with_deduction(
    args: &[Option<ValueRef>],
    db: &FactDB,
) -> Option<Fact> {
    if args.len() != 2 {
        return None;
    }

    let deduced_args = match (&args[0], &args[1]) {
        // ?X == bound -> bind ?X
        (None, Some(vr1)) => {
            if let ValueRef::Key(_) = vr1 {
                // If the bound argument is an anchored key, we can set the
                // wildcard to its *value*, but only if we know it.
                db.value_ref_to_value(vr1)
                    .map(|value| vec![ValueRef::from(value.clone()), vr1.clone()])
            } else {
                Some(vec![vr1.clone(), vr1.clone()])
            }
        }
        // bound == ?Y -> bind ?Y
        (Some(vr0), None) => {
            if let ValueRef::Key(_) = vr0 {
                db.value_ref_to_value(vr0)
                    .map(|value| vec![vr0.clone(), ValueRef::from(value.clone())])
            } else {
                // The bound argument is a value, so we can set the wildcard to it directly.
                Some(vec![vr0.clone(), vr0.clone()])
            }
        }
        // Both sides already bound – nothing to deduce here.
        _ => None,
    }?;

    Some(Fact {
        source: FactSource::Native(NativeOperation::EqualFromEntries),
        args: deduced_args,
    })
}

fn materialize_not_equal_from_entries(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 2 {
        return None;
    }

    let (vr0, vr1) = match (&args[0], &args[1]) {
        (Some(vr0), Some(vr1)) => (vr0, vr1),
        _ => return None, // Both args must be bound for value comparison
    };

    // Can both args be resolved to values?
    if let (Some(val0), Some(val1)) = (db.value_ref_to_value(vr0), db.value_ref_to_value(vr1)) {
        // Do direct value comparison
        if val0 != val1 {
            Some(Fact {
                source: FactSource::Native(NativeOperation::NotEqualFromEntries),
                args: vec![vr0.clone(), vr1.clone()],
            })
        } else {
            None
        }
    } else {
        // Some arguments are anchored keys we can't resolve
        // CopyStatement will handle looking up existing statements
        None
    }
}

fn materialize_lt_eq_from_entries(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 2 {
        return None;
    }

    let (vr0, vr1) = match (&args[0], &args[1]) {
        (Some(vr0), Some(vr1)) => (vr0, vr1),
        _ => return None, // Both args must be bound for value comparison
    };

    // Can both args be resolved to values?
    if let (Some(val0), Some(val1)) = (db.value_ref_to_value(vr0), db.value_ref_to_value(vr1)) {
        // Do direct value comparison for integers
        if let (TypedValue::Int(i1), TypedValue::Int(i2)) = (val0.typed(), val1.typed()) {
            if i1 <= i2 {
                Some(Fact {
                    source: FactSource::Native(NativeOperation::LtEqFromEntries),
                    args: vec![vr0.clone(), vr1.clone()],
                })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        // Some arguments are anchored keys we can't resolve
        // CopyStatement will handle looking up existing statements
        None
    }
}

fn materialize_lt_from_entries(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 2 {
        return None;
    }

    let (vr0, vr1) = match (&args[0], &args[1]) {
        (Some(vr0), Some(vr1)) => (vr0, vr1),
        _ => return None, // Both args must be bound for value comparison
    };

    // Can both args be resolved to values?
    if let (Some(val0), Some(val1)) = (db.value_ref_to_value(vr0), db.value_ref_to_value(vr1)) {
        // Do direct value comparison for integers
        if let (TypedValue::Int(i1), TypedValue::Int(i2)) = (val0.typed(), val1.typed()) {
            if i1 < i2 {
                Some(Fact {
                    source: FactSource::Native(NativeOperation::LtFromEntries),
                    args: vec![vr0.clone(), vr1.clone()],
                })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        // Some arguments are anchored keys we can't resolve
        // CopyStatement will handle looking up existing statements
        None
    }
}

fn materialize_transitive_equal_from_statements(
    args: &[Option<ValueRef>],
    db: &FactDB,
) -> Option<Fact> {
    if args.len() != 2 {
        return None;
    }

    let (vr0, vr1) = match (&args[0], &args[1]) {
        (Some(vr0), Some(vr1)) => (vr0, vr1),
        _ => return None, // Both args must be bound for transitive equality
    };

    // Transitive equality only works with anchored keys
    if let (ValueRef::Key(key0), ValueRef::Key(key1)) = (vr0, vr1) {
        if let Some(path) = db.find_path_and_nodes(key0, key1) {
            if path.len() > 2 {
                // If the path length is 2 (A and B), we don't need transitive equality.
                Some(Fact {
                    source: FactSource::Special,
                    args: vec![vr0.clone(), vr1.clone()],
                })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

fn materialize_lt_to_not_equal(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 2 {
        return None;
    }

    let (vr0, vr1) = match (&args[0], &args[1]) {
        (Some(vr0), Some(vr1)) => (vr0, vr1),
        _ => return None, // Both args must be bound
    };

    // LtToNotEqual is a statement derivation operation: Lt(A,B) → NotEqual(A,B)
    // We need to check if Lt(A,B) exists to derive NotEqual(A,B)
    if let Some(index) = db.get_binary_statement_index(&NativePredicate::Lt) {
        if index.contains_key(&[vr0.clone(), vr1.clone()]) {
            Some(Fact {
                source: FactSource::Native(NativeOperation::LtToNotEqual),
                args: vec![vr0.clone(), vr1.clone()],
            })
        } else {
            None
        }
    } else {
        None
    }
}

fn materialize_contains_from_entries(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 3 {
        return None;
    }

    // Try to deduce with free args first
    if let Some(deduced_fact) = materialize_contains_with_deduction(args, db) {
        return Some(deduced_fact);
    }

    // Are all args bound?
    let (vr0, vr1, vr2) = match (&args[0], &args[1], &args[2]) {
        (Some(vr0), Some(vr1), Some(vr2)) => (vr0, vr1, vr2),
        _ => return None,
    };

    // Can all args be resolved to values?
    if let (Some(val0), Some(val1), Some(val2)) = (
        db.value_ref_to_value(vr0),
        db.value_ref_to_value(vr1),
        db.value_ref_to_value(vr2),
    ) {
        // Do value-based check
        let contains = match val0.typed() {
            TypedValue::Array(arr) => {
                if let TypedValue::Int(idx) = val1.typed() {
                    if let Ok(i) = usize::try_from(*idx) {
                        arr.get(i).is_ok_and(|v| v == &val2)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            TypedValue::Dictionary(dict) => {
                if let TypedValue::String(s) = val1.typed() {
                    dict.get(&Key::new(s.clone())).is_ok_and(|v| v == &val2)
                } else {
                    false
                }
            }
            TypedValue::Set(set) => {
                // For a set, key and value must be the same
                val1 == val2 && set.contains(&val1)
            }
            _ => false,
        };

        if contains {
            Some(Fact {
                source: FactSource::Native(NativeOperation::ContainsFromEntries),
                args: vec![vr0.clone(), vr1.clone(), vr2.clone()],
            })
        } else {
            None
        }
    } else {
        // Some arguments are anchored keys we can't resolve
        // CopyStatement will handle looking up existing statements
        None
    }
}

/// Helper function for Contains deduction when some args are unbound
fn materialize_contains_with_deduction(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    match (&args[0], &args[1], &args[2]) {
        (Some(vr1), Some(vr2), None) => {
            if let (Some(val1), Some(val2)) =
                (db.value_ref_to_value(vr1), db.value_ref_to_value(vr2))
            {
                if let TypedValue::Array(arr) = val1.typed() {
                    if let TypedValue::Int(idx) = val2.typed() {
                        if let Ok(i) = usize::try_from(*idx) {
                            if let Ok(val) = arr.get(i) {
                                return Some(Fact {
                                    source: FactSource::Native(
                                        NativeOperation::ContainsFromEntries,
                                    ),
                                    args: vec![
                                        vr1.clone(),
                                        vr2.clone(),
                                        ValueRef::from(val.clone()),
                                    ],
                                });
                            }
                        }
                    }
                }
                if let TypedValue::Dictionary(dict) = val1.typed() {
                    if let TypedValue::String(s) = val2.typed() {
                        if let Ok(val) = dict.get(&Key::new(s.clone())) {
                            return Some(Fact {
                                source: FactSource::Native(NativeOperation::ContainsFromEntries),
                                args: vec![vr1.clone(), vr2.clone(), ValueRef::from(val.clone())],
                            });
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn materialize_not_contains_from_entries(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 2 {
        return None;
    }

    let (vr0, vr1) = match (&args[0], &args[1]) {
        (Some(vr0), Some(vr1)) => (vr0, vr1),
        _ => return None, // Both args must be bound for value comparison
    };

    // Can both args be resolved to values?
    if let (Some(val0), Some(val1)) = (db.value_ref_to_value(vr0), db.value_ref_to_value(vr1)) {
        // Do value-based check
        let not_contains = match val0.typed() {
            TypedValue::Array(arr) => {
                if let TypedValue::Int(idx) = val1.typed() {
                    if let Ok(i) = usize::try_from(*idx) {
                        arr.get(i).is_err()
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            TypedValue::Dictionary(dict) => {
                if let TypedValue::String(s) = val1.typed() {
                    dict.get(&Key::new(s.clone())).is_err()
                } else {
                    false
                }
            }
            TypedValue::Set(set) => !set.contains(&val1),
            _ => false,
        };

        if not_contains {
            Some(Fact {
                source: FactSource::Native(NativeOperation::NotContainsFromEntries),
                args: vec![vr0.clone(), vr1.clone()],
            })
        } else {
            None
        }
    } else {
        // Some arguments are anchored keys we can't resolve
        // CopyStatement will handle looking up existing statements
        None
    }
}

fn materialize_sum_of(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 3 {
        return None;
    }

    let int = |vr: &ValueRef| {
        db.value_ref_to_value(vr).and_then(|v| match v.typed() {
            TypedValue::Int(i) => Some(*i),
            _ => None,
        })
    };

    // Try deduction with free args first
    let deduced_args = match (&args[0], &args[1], &args[2]) {
        // SumOf(?x, 5, 10) -> x = 15
        (None, Some(vr1), Some(vr2)) => {
            if let (Some(i1), Some(i2)) = (int(vr1), int(vr2)) {
                Some(vec![ValueRef::from(i1 + i2), vr1.clone(), vr2.clone()])
            } else {
                None
            }
        }
        // SumOf(15, ?y, 10) -> y = 5
        (Some(vr0), None, Some(vr2)) => {
            if let (Some(i0), Some(i2)) = (int(vr0), int(vr2)) {
                Some(vec![vr0.clone(), ValueRef::from(i0 - i2), vr2.clone()])
            } else {
                None
            }
        }
        // SumOf(15, 5, ?z) -> z = 10
        (Some(vr0), Some(vr1), None) => {
            if let (Some(i0), Some(i1)) = (int(vr0), int(vr1)) {
                Some(vec![vr0.clone(), vr1.clone(), ValueRef::from(i0 - i1)])
            } else {
                None
            }
        }
        // All args bound - do value check
        (Some(vr0), Some(vr1), Some(vr2)) => {
            if let (Some(i0), Some(i1), Some(i2)) = (int(vr0), int(vr1), int(vr2)) {
                if i0 == i1 + i2 {
                    Some(vec![vr0.clone(), vr1.clone(), vr2.clone()])
                } else {
                    None
                }
            } else {
                // Can't resolve all to ints
                // CopyStatement will handle looking up existing statements
                None
            }
        }
        _ => None,
    };

    deduced_args.map(|args| Fact {
        source: FactSource::Native(NativeOperation::SumOf),
        args,
    })
}

fn materialize_product_of(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 3 {
        return None;
    }

    let int = |vr: &ValueRef| {
        db.value_ref_to_value(vr).and_then(|v| match v.typed() {
            TypedValue::Int(i) => Some(*i),
            _ => None,
        })
    };

    // Try deduction with free args first
    let deduced_args = match (&args[0], &args[1], &args[2]) {
        // ProductOf(?x, 5, 10) -> x = 50
        (None, Some(vr1), Some(vr2)) => {
            if let (Some(i1), Some(i2)) = (int(vr1), int(vr2)) {
                Some(vec![ValueRef::from(i1 * i2), vr1.clone(), vr2.clone()])
            } else {
                None
            }
        }
        // ProductOf(50, ?y, 10) -> y = 5
        (Some(vr0), None, Some(vr2)) => {
            if let (Some(i0), Some(i2)) = (int(vr0), int(vr2)) {
                if i2 != 0 {
                    Some(vec![vr0.clone(), ValueRef::from(i0 / i2), vr2.clone()])
                } else {
                    None
                }
            } else {
                None
            }
        }
        // ProductOf(50, 5, ?z) -> z = 10
        (Some(vr0), Some(vr1), None) => {
            if let (Some(i0), Some(i1)) = (int(vr0), int(vr1)) {
                if i1 != 0 {
                    Some(vec![vr0.clone(), vr1.clone(), ValueRef::from(i0 / i1)])
                } else {
                    None
                }
            } else {
                None
            }
        }
        // All args bound - do value check
        (Some(vr0), Some(vr1), Some(vr2)) => {
            if let (Some(i0), Some(i1), Some(i2)) = (int(vr0), int(vr1), int(vr2)) {
                if i0 == i1 * i2 {
                    Some(vec![vr0.clone(), vr1.clone(), vr2.clone()])
                } else {
                    None
                }
            } else {
                // Can't resolve all to ints
                // CopyStatement will handle looking up existing statements
                None
            }
        }
        _ => None,
    };

    deduced_args.map(|args| Fact {
        source: FactSource::Native(NativeOperation::ProductOf),
        args,
    })
}

fn materialize_max_of(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 3 {
        return None;
    }

    let int = |vr: &ValueRef| {
        db.value_ref_to_value(vr).and_then(|v| match v.typed() {
            TypedValue::Int(i) => Some(*i),
            _ => None,
        })
    };

    // Try deduction with free args first
    let deduced_args = match (&args[0], &args[1], &args[2]) {
        // MaxOf(?x, 5, 10) -> x = 10
        (None, Some(vr1), Some(vr2)) => {
            if let (Some(i1), Some(i2)) = (int(vr1), int(vr2)) {
                Some(vec![ValueRef::from(i1.max(i2)), vr1.clone(), vr2.clone()])
            } else {
                None
            }
        }
        // MaxOf(10, ?y, 10) -> y could be anything <= 10, but we'll use the max value for consistency
        (Some(vr0), None, Some(vr2)) => {
            if let (Some(i0), Some(i2)) = (int(vr0), int(vr2)) {
                Some(vec![vr0.clone(), ValueRef::from(i0.max(i2)), vr2.clone()])
            } else {
                None
            }
        }
        // MaxOf(10, 10, ?z) -> z could be anything <= 10
        (Some(vr0), Some(vr1), None) => {
            if let (Some(i0), Some(i1)) = (int(vr0), int(vr1)) {
                Some(vec![vr0.clone(), vr1.clone(), ValueRef::from(i0.max(i1))])
            } else {
                None
            }
        }
        // All args bound - do value check
        (Some(vr0), Some(vr1), Some(vr2)) => {
            if let (Some(i0), Some(i1), Some(i2)) = (int(vr0), int(vr1), int(vr2)) {
                if i0 == i1.max(i2) {
                    Some(vec![vr0.clone(), vr1.clone(), vr2.clone()])
                } else {
                    None
                }
            } else {
                // Can't resolve all to ints
                // CopyStatement will handle looking up existing statements
                None
            }
        }
        _ => None,
    };

    deduced_args.map(|args| Fact {
        source: FactSource::Native(NativeOperation::MaxOf),
        args,
    })
}

fn materialize_hash_of(args: &[Option<ValueRef>], db: &FactDB) -> Option<Fact> {
    if args.len() != 3 {
        return None;
    }

    // Try deduction with free args first
    let deduced_args = match (&args[0], &args[1], &args[2]) {
        // HashOf(?x, 5, 10) -> x = hash(5, 10)
        (None, Some(vr1), Some(vr2)) => {
            if let (Some(val1), Some(val2)) =
                (db.value_ref_to_value(vr1), db.value_ref_to_value(vr2))
            {
                Some(vec![
                    ValueRef::from(hash_values(&[val1, val2])),
                    vr1.clone(),
                    vr2.clone(),
                ])
            } else {
                None
            }
        }
        // HashOf(hash_val, ?y, 10) -> y = ? (cannot easily reverse hash)
        (Some(_vr0), None, Some(_vr2)) => {
            // Cannot easily reverse hash functions, so we don't support this deduction
            None
        }
        // HashOf(hash_val, 5, ?z) -> z = ? (cannot easily reverse hash)
        (Some(_vr0), Some(_vr1), None) => {
            // Cannot easily reverse hash functions, so we don't support this deduction
            None
        }
        // All args bound - do value check
        (Some(vr0), Some(vr1), Some(vr2)) => {
            if let (Some(val0), Some(val1), Some(val2)) = (
                db.value_ref_to_value(vr0),
                db.value_ref_to_value(vr1),
                db.value_ref_to_value(vr2),
            ) {
                let hash_val = Value::from(hash_values(&[val1, val2]));
                if val0 == hash_val {
                    Some(vec![vr0.clone(), vr1.clone(), vr2.clone()])
                } else {
                    None
                }
            } else {
                // Can't resolve all to values
                // CopyStatement will handle looking up existing statements
                None
            }
        }
        _ => None,
    };

    deduced_args.map(|args| Fact {
        source: FactSource::Native(NativeOperation::HashOf),
        args,
    })
}

fn materialize_public_key_of(args: &[Option<ValueRef>], db: &FactDB) -> Relation {
    if args.len() != 2 {
        return Relation::new();
    }

    match (&args[0], &args[1]) {
        // Both PK and SK are bound: check if they match.
        (Some(vr0), Some(vr1)) => {
            if let (Some(val0), Some(val1)) =
                (db.value_ref_to_value(vr0), db.value_ref_to_value(vr1))
            {
                if let (TypedValue::PublicKey(pk), TypedValue::SecretKey(sk)) =
                    (val0.typed(), val1.typed())
                {
                    if sk.public_key() == *pk {
                        return std::iter::once(Fact {
                            source: FactSource::Native(NativeOperation::PublicKeyOf),
                            args: vec![vr0.clone(), vr1.clone()],
                        })
                        .collect();
                    }
                }
            }
            Relation::new()
        }

        // Only SK is bound: deduce PK.
        (None, Some(vr1)) => {
            if let Some(val1) = db.value_ref_to_value(vr1) {
                if let TypedValue::SecretKey(sk) = val1.typed() {
                    let pk = sk.public_key();
                    let pk_val = Value::from(TypedValue::PublicKey(pk));
                    return std::iter::once(Fact {
                        source: FactSource::Native(NativeOperation::PublicKeyOf),
                        args: vec![ValueRef::Literal(pk_val), vr1.clone()],
                    })
                    .collect();
                }
            }
            Relation::new()
        }

        // Unbound or only PK is bound: iterate all known keypairs.
        _ => db
            .keypairs_iter()
            .filter_map(|sk| {
                let pk = sk.public_key();
                let pk_val = Value::from(TypedValue::PublicKey(pk));
                let sk_val = Value::from(TypedValue::SecretKey(sk.clone()));
                let pk_vr = ValueRef::Literal(pk_val);
                let sk_vr = ValueRef::Literal(sk_val);

                // If PK is bound, check if it matches.
                if let Some(vr0) = &args[0] {
                    if vr0 != &pk_vr {
                        return None;
                    }
                }

                Some(Fact {
                    source: FactSource::Native(NativeOperation::PublicKeyOf),
                    args: vec![pk_vr, sk_vr],
                })
            })
            .collect(),
    }
}

fn explain_transitive_equal_from_statements(
    args: &[ValueRef],
    db: &FactDB,
) -> Result<Vec<Operation>, SolverError> {
    if args.len() != 2 {
        return Ok(vec![]);
    }

    if let (ValueRef::Key(key0), ValueRef::Key(key1)) = (&args[0], &args[1]) {
        if let Some(path) = db.find_path_and_nodes(key0, key1) {
            // Build a transitive-equality operation for every triple (A,B,C) along the path
            // A==B & B==C  =>  transitive_eq(A,B,C)
            let ops_res: Result<Vec<Operation>, SolverError> = path
                .windows(3)
                .map(|w| {
                    let (k0, k1, k2) = (w[0].clone(), w[1].clone(), w[2].clone());
                    let left_args = [ValueRef::Key(k0.clone()), ValueRef::Key(k1.clone())];
                    let right_args = [ValueRef::Key(k1), ValueRef::Key(k2)];

                    // Ensure both required Equal statements are asserted; otherwise bail.
                    if let Some(idx) = db.get_binary_statement_index(&NativePredicate::Equal) {
                        if idx.contains_key(&left_args) && idx.contains_key(&right_args) {
                            Ok(Operation::transitive_eq(
                                &Statement::Equal(left_args[0].clone(), left_args[1].clone()),
                                &Statement::Equal(right_args[0].clone(), right_args[1].clone()),
                            ))
                        } else {
                            Err(SolverError::Internal(
                                "Equality path contains non-equal statements".to_string(),
                            ))
                        }
                    } else {
                        Err(SolverError::Internal(
                            "Equality index missing during proof construction".to_string(),
                        ))
                    }
                })
                .collect();

            return ops_res;
        }
    }
    Err(SolverError::Internal(
        "Equality path not found during proof construction".to_string(),
    ))
}
