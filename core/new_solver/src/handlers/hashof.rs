use pod2::middleware::{hash_values, StatementTmplArg, Value};
use tracing::trace;

use super::util::{arg_to_selector, handle_copy_results};
use crate::{
    edb::EdbView,
    op::OpHandler,
    prop::{Choice, PropagatorResult},
    types::{ConstraintStore, OpTag},
};

/// Classification of an argument for HashOf operations
#[derive(Debug)]
enum HashArg {
    /// Concrete value (from literal or bound wildcard)
    Bound(Value),
    /// Unbound wildcard that needs to be filled
    Unbound(usize), // wildcard index
    /// Unsupported argument type
    Unsupported,
}

/// Classify an argument for HashOf processing
fn classify_arg(arg: &StatementTmplArg, store: &ConstraintStore) -> HashArg {
    match arg {
        StatementTmplArg::Literal(v) => HashArg::Bound(v.clone()),
        StatementTmplArg::Wildcard(w) => {
            if let Some(v) = store.bindings.get(&w.index) {
                HashArg::Bound(v.clone())
            } else {
                HashArg::Unbound(w.index)
            }
        }
        _ => HashArg::Unsupported,
    }
}

/// HashOf from literals/entries: supports hash computation and validation.
/// Semantics: a = hash([b, c])
/// Only supports inference of `a` when `b` and `c` are known.
/// Cannot reverse the hash function to infer `b` or `c` from `a`.
pub struct HashOfFromEntriesHandler;

impl OpHandler for HashOfFromEntriesHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        _edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 3 {
            return PropagatorResult::Contradiction;
        }

        trace!("HashOf: start args_len=3");
        let a = classify_arg(&args[0], store);
        let b = classify_arg(&args[1], store);
        let c = classify_arg(&args[2], store);
        trace!("HashOf: classified A={:?} B={:?} C={:?}", a, b, c);

        match (&a, &b, &c) {
            (HashArg::Unsupported, _, _)
            | (_, HashArg::Unsupported, _)
            | (_, _, HashArg::Unsupported) => {
                trace!("HashOf: unsupported argument type -> contradiction");
                PropagatorResult::Contradiction
            }
            // All bound: validate a == hash([b, c])
            (HashArg::Bound(val_a), HashArg::Bound(val_b), HashArg::Bound(val_c)) => {
                let expected_hash = Value::from(hash_values(&[val_b.clone(), val_c.clone()]));
                if val_a == &expected_hash {
                    trace!("HashOf: all bound - validation passed");
                    PropagatorResult::Choices {
                        alternatives: vec![Choice {
                            bindings: vec![],
                            op_tag: OpTag::FromLiterals,
                        }],
                    }
                } else {
                    trace!("HashOf: all bound - validation failed");
                    PropagatorResult::Contradiction
                }
            }
            // Unknown a, known b and c: a = hash([b, c])
            (HashArg::Unbound(a_idx), HashArg::Bound(val_b), HashArg::Bound(val_c)) => {
                let hash_val = Value::from(hash_values(&[val_b.clone(), val_c.clone()]));
                trace!("HashOf: computing hash for a={}", a_idx);
                PropagatorResult::Choices {
                    alternatives: vec![Choice {
                        bindings: vec![(*a_idx, hash_val)],
                        op_tag: OpTag::FromLiterals,
                    }],
                }
            }
            // Unknown b or c: cannot reverse hash, so suspend
            (HashArg::Bound(_), HashArg::Unbound(b_idx), HashArg::Bound(_)) => {
                trace!("HashOf: cannot reverse hash to find b -> suspend");
                PropagatorResult::Suspend { on: vec![*b_idx] }
            }
            (HashArg::Bound(_), HashArg::Bound(_), HashArg::Unbound(c_idx)) => {
                trace!("HashOf: cannot reverse hash to find c -> suspend");
                PropagatorResult::Suspend { on: vec![*c_idx] }
            }
            (HashArg::Bound(_), HashArg::Unbound(b_idx), HashArg::Unbound(c_idx)) => {
                trace!("HashOf: cannot reverse hash to find b,c -> suspend");
                PropagatorResult::Suspend {
                    on: vec![*b_idx, *c_idx],
                }
            }
            // Multiple unknowns including a: suspend on all wildcards
            (HashArg::Unbound(a_idx), HashArg::Unbound(b_idx), HashArg::Bound(_)) => {
                trace!("HashOf: multiple unknowns a,b -> suspend");
                PropagatorResult::Suspend {
                    on: vec![*a_idx, *b_idx],
                }
            }
            (HashArg::Unbound(a_idx), HashArg::Bound(_), HashArg::Unbound(c_idx)) => {
                trace!("HashOf: multiple unknowns a,c -> suspend");
                PropagatorResult::Suspend {
                    on: vec![*a_idx, *c_idx],
                }
            }
            (HashArg::Unbound(a_idx), HashArg::Unbound(b_idx), HashArg::Unbound(c_idx)) => {
                trace!("HashOf: all unknowns -> suspend");
                PropagatorResult::Suspend {
                    on: vec![*a_idx, *b_idx, *c_idx],
                }
            }
        }
    }
}

/// Copy HashOf statements from EDB with wildcard substitution.
/// Binds unbound arguments from existing HashOf facts in the EDB.
pub struct CopyHashOfHandler;

impl OpHandler for CopyHashOfHandler {
    fn propagate(
        &self,
        args: &[StatementTmplArg],
        store: &mut ConstraintStore,
        edb: &dyn EdbView,
    ) -> PropagatorResult {
        if args.len() != 3 {
            return PropagatorResult::Contradiction;
        }

        // We need to store owned values for selectors, since ArgSel holds references.
        let (mut a_val, mut a_root) = (None, None);
        let (mut b_val, mut b_root) = (None, None);
        let (mut c_val, mut c_root) = (None, None);

        let sel_a = arg_to_selector(&args[0], store, &mut a_val, &mut a_root);
        let sel_b = arg_to_selector(&args[1], store, &mut b_val, &mut b_root);
        let sel_c = arg_to_selector(&args[2], store, &mut c_val, &mut c_root);

        let results = edb.query(
            crate::edb::PredicateKey::Native(pod2::middleware::NativePredicate::HashOf),
            &[sel_a, sel_b, sel_c],
        );

        handle_copy_results(results, args, store)
    }
}

pub fn register_hashof_handlers(reg: &mut crate::op::OpRegistry) {
    use pod2::middleware::NativePredicate;

    reg.register(NativePredicate::HashOf, Box::new(HashOfFromEntriesHandler));
    reg.register(NativePredicate::HashOf, Box::new(CopyHashOfHandler));
}

#[cfg(test)]
mod tests {
    use pod2::{
        lang::PrettyPrint,
        middleware::{Statement, StatementTmplArg, Value},
    };

    use super::*;
    use crate::{
        edb::ImmutableEdbBuilder,
        test_helpers::{self, args_from},
        types::ConstraintStore,
    };

    #[test]
    fn hashof_compute_hash() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = HashOfFromEntriesHandler;

        let args = args_from("REQUEST(HashOf(?X, \"hello\", \"world\"))");
        let result = handler.propagate(&args, &mut store, &edb);

        match result {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                assert_eq!(alternatives[0].bindings.len(), 1);
                // Check that we got a hash binding for wildcard 0
                let expected_hash =
                    Value::from(hash_values(&[Value::from("hello"), Value::from("world")]));
                assert_eq!(alternatives[0].bindings[0], (0, expected_hash));
            }
            _ => panic!("Expected choices, got {result:?}"),
        }
    }

    #[test]
    fn hashof_all_ground_invalid() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = HashOfFromEntriesHandler;

        let args = args_from("REQUEST(HashOf(\"wrong_hash\", \"hello\", \"world\"))");
        let result = handler.propagate(&args, &mut store, &edb);
        assert!(matches!(result, PropagatorResult::Contradiction));
    }

    #[test]
    fn hashof_unknown_b_suspends() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = HashOfFromEntriesHandler;

        let args = args_from("REQUEST(HashOf(\"some_hash\", ?Y, \"world\"))");
        let result = handler.propagate(&args, &mut store, &edb);
        // ?Y is the first wildcard, so it gets index 0
        assert!(matches!(result, PropagatorResult::Suspend { on } if on == vec![0]));
    }

    #[test]
    fn hashof_unknown_c_suspends() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = HashOfFromEntriesHandler;

        let args = args_from("REQUEST(HashOf(\"some_hash\", \"hello\", ?Z))");
        let result = handler.propagate(&args, &mut store, &edb);
        // ?Z is the first wildcard, so it gets index 0
        assert!(matches!(result, PropagatorResult::Suspend { on } if on == vec![0]));
    }

    #[test]
    fn copy_hashof_validates_all_ground() {
        let mut store = ConstraintStore::default();
        let handler = CopyHashOfHandler;
        let src = crate::types::PodRef(test_helpers::root("s"));

        let val_b = Value::from("hello");
        let val_c = Value::from("world");
        let hash_val = Value::from(hash_values(&[val_b.clone(), val_c.clone()]));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::HashOf(hash_val.clone().into(), val_b.into(), val_c.into()),
                src,
            )
            .build();

        // Copy handler should validate all-ground facts
        let args = args_from(&format!(
            "REQUEST(HashOf({}, \"hello\", \"world\"))",
            hash_val.to_podlang_string()
        ));
        let result = handler.propagate(&args, &mut store, &edb);

        match result {
            PropagatorResult::Choices { alternatives } => {
                assert_eq!(alternatives.len(), 1);
                assert_eq!(alternatives[0].bindings.len(), 0); // No wildcards to bind
                assert!(matches!(
                    alternatives[0].op_tag,
                    OpTag::CopyStatement { .. }
                ));
            }
            _ => panic!("Expected choices, got {result:?}"),
        }
    }

    #[test]
    fn copy_hashof_matches_two_of_three_and_binds_third() {
        let src = crate::types::PodRef(test_helpers::root("s"));
        let hash_val = Value::from(hash_values(&[Value::from("hello"), Value::from("world")]));
        let edb = ImmutableEdbBuilder::new()
            .add_statement_for_test(
                Statement::HashOf(hash_val.clone().into(), "hello".into(), "world".into()),
                src,
            )
            .build();
        let mut store = ConstraintStore::default();
        let handler = CopyHashOfHandler;
        // Match first two, bind third
        let args = args_from("REQUEST(HashOf(?X, \"hello\", \"world\"))");
        let res = handler.propagate(&args, &mut store, &edb);
        match res {
            PropagatorResult::Choices { alternatives } => {
                assert!(alternatives
                    .iter()
                    .any(|ch| ch.bindings.iter().any(|(i, v)| *i == 0 && *v == hash_val)));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn copy_hashof_no_match_contradiction() {
        let edb = ImmutableEdbBuilder::new().build(); // Empty EDB
        let mut store = ConstraintStore::default();
        let handler = CopyHashOfHandler;

        let args = args_from("REQUEST(HashOf(\"some_hash\", \"hello\", \"world\"))");
        let result = handler.propagate(&args, &mut store, &edb);
        assert!(matches!(result, PropagatorResult::Contradiction));
    }

    #[test]
    fn hashof_wrong_argument_count_contradiction() {
        let edb = ImmutableEdbBuilder::new().build();
        let mut store = ConstraintStore::default();
        let handler = HashOfFromEntriesHandler;

        // Manually create args with wrong count (2 instead of 3)
        let args = vec![
            StatementTmplArg::Literal(Value::from("hello")),
            StatementTmplArg::Literal(Value::from("world")),
        ];

        let result = handler.propagate(&args, &mut store, &edb);
        assert!(matches!(result, PropagatorResult::Contradiction));
    }
}
