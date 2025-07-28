//! Unit tests for OperationMaterializer system
//!
//! This module tests the Operation-centric materializer architecture that replaced
//! the legacy PredicateHandler system. The tests validate the three categories of operations:
//!
//! 1. **Value-based computations** - Pure value computation without statement lookups
//! 2. **Statement copying** - CopyStatement as the primary lookup mechanism  
//! 3. **Statement derivations** - Logical inference operations with targeted lookups

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pod2::middleware::{hash_str, AnchoredKey, Key, NativePredicate, PodId, Value, ValueRef};

    use crate::{
        db::FactDB, engine::semi_naive::FactSource,
        semantics::operation_materializers::OperationMaterializer,
    };

    // Test Infrastructure Helpers
    fn create_test_db() -> FactDB {
        FactDB::new()
    }

    fn val_int(i: i64) -> Value {
        Value::from(i)
    }

    fn val_str(s: &str) -> Value {
        Value::from(s)
    }

    fn val_ref_int(i: i64) -> ValueRef {
        ValueRef::Literal(val_int(i))
    }

    fn val_ref_str(s: &str) -> ValueRef {
        ValueRef::Literal(val_str(s))
    }

    fn val_ref_key(pod_id: PodId, key_name: &str) -> ValueRef {
        ValueRef::Key(AnchoredKey::new(pod_id, Key::new(key_name.to_string())))
    }

    // ================================================================================================
    // Tests for Value-Based Computation Operations
    // ================================================================================================

    #[test]
    fn test_equal_from_entries_both_bound_same() {
        let db = create_test_db();
        let materializer = OperationMaterializer::EqualFromEntries;

        let args = vec![Some(val_ref_int(42)), Some(val_ref_int(42))];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Native(_)));
        assert_eq!(fact.args.len(), 2);
    }

    #[test]
    fn test_equal_from_entries_both_bound_different() {
        let db = create_test_db();
        let materializer = OperationMaterializer::EqualFromEntries;

        let args = vec![Some(val_ref_int(42)), Some(val_ref_int(24))];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    #[test]
    fn test_equal_from_entries_one_bound_literal() {
        let db = create_test_db();
        let materializer = OperationMaterializer::EqualFromEntries;

        // One arg bound, one unbound - should deduce that both are equal to the bound value
        let args = vec![Some(val_ref_int(42)), None];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], val_ref_int(42));
        assert_eq!(fact.args[1], val_ref_int(42));
    }

    #[test]
    fn test_equal_from_entries_unbound() {
        let db = create_test_db();
        let materializer = OperationMaterializer::EqualFromEntries;

        let args = vec![None, None];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    #[test]
    fn test_not_equal_from_entries_both_bound_different() {
        let db = create_test_db();
        let materializer = OperationMaterializer::NotEqualFromEntries;

        let args = vec![Some(val_ref_int(42)), Some(val_ref_int(24))];
        let result = materializer.materialize(&args, &db, NativePredicate::NotEqual);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Native(_)));
    }

    #[test]
    fn test_not_equal_from_entries_both_bound_same() {
        let db = create_test_db();
        let materializer = OperationMaterializer::NotEqualFromEntries;

        let args = vec![Some(val_ref_int(42)), Some(val_ref_int(42))];
        let result = materializer.materialize(&args, &db, NativePredicate::NotEqual);

        assert!(result.is_none());
    }

    #[test]
    fn test_lt_from_entries_valid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtFromEntries;

        let args = vec![Some(val_ref_int(10)), Some(val_ref_int(20))];
        let result = materializer.materialize(&args, &db, NativePredicate::Lt);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Native(_)));
        assert_eq!(fact.args[0], val_ref_int(10));
        assert_eq!(fact.args[1], val_ref_int(20));
    }

    #[test]
    fn test_lt_from_entries_invalid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtFromEntries;

        // 20 is not less than 10
        let args = vec![Some(val_ref_int(20)), Some(val_ref_int(10))];
        let result = materializer.materialize(&args, &db, NativePredicate::Lt);

        assert!(result.is_none());
    }

    #[test]
    fn test_lt_from_entries_equal_values() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtFromEntries;

        // Equal values should not satisfy less-than
        let args = vec![Some(val_ref_int(15)), Some(val_ref_int(15))];
        let result = materializer.materialize(&args, &db, NativePredicate::Lt);

        assert!(result.is_none());
    }

    #[test]
    fn test_lt_from_entries_partially_bound() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtFromEntries;

        let args = vec![Some(val_ref_int(10)), None];
        let result = materializer.materialize(&args, &db, NativePredicate::Lt);

        assert!(result.is_none());
    }

    #[test]
    fn test_lt_eq_from_entries_valid_less_than() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtEqFromEntries;

        let args = vec![Some(val_ref_int(10)), Some(val_ref_int(20))];
        let result = materializer.materialize(&args, &db, NativePredicate::LtEq);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Native(_)));
        assert_eq!(fact.args[0], val_ref_int(10));
        assert_eq!(fact.args[1], val_ref_int(20));
    }

    #[test]
    fn test_lt_eq_from_entries_valid_equal() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtEqFromEntries;

        let args = vec![Some(val_ref_int(15)), Some(val_ref_int(15))];
        let result = materializer.materialize(&args, &db, NativePredicate::LtEq);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Native(_)));
        assert_eq!(fact.args[0], val_ref_int(15));
        assert_eq!(fact.args[1], val_ref_int(15));
    }

    #[test]
    fn test_lt_eq_from_entries_invalid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtEqFromEntries;

        // 20 is not less than or equal to 10
        let args = vec![Some(val_ref_int(20)), Some(val_ref_int(10))];
        let result = materializer.materialize(&args, &db, NativePredicate::LtEq);

        assert!(result.is_none());
    }

    #[test]
    fn test_lt_eq_from_entries_with_different_predicate() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtEqFromEntries;

        // LtEq operation works based on value logic, not predicate parameter
        // The predicate parameter is used mainly for operations like CopyStatement
        let args = vec![Some(val_ref_int(10)), Some(val_ref_int(20))];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        // This will succeed because 10 <= 20, regardless of predicate parameter
        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], val_ref_int(10));
        assert_eq!(fact.args[1], val_ref_int(20));
    }

    #[test]
    fn test_contains_from_entries_wrong_argument_count() {
        let db = create_test_db();
        let materializer = OperationMaterializer::ContainsFromEntries;

        // Contains requires 3 arguments
        let args = vec![Some(val_ref_int(1)), Some(val_ref_int(2))]; // Only 2 args
        let result = materializer.materialize(&args, &db, NativePredicate::Contains);

        assert!(result.is_none());
    }

    #[test]
    fn test_contains_from_entries_partially_bound() {
        let db = create_test_db();
        let materializer = OperationMaterializer::ContainsFromEntries;

        // Partially bound args - missing array value for proper testing
        let args = vec![None, Some(val_ref_int(0)), Some(val_ref_int(42))];
        let result = materializer.materialize(&args, &db, NativePredicate::Contains);

        assert!(result.is_none());
    }

    #[test]
    fn test_not_contains_from_entries_wrong_argument_count() {
        let db = create_test_db();
        let materializer = OperationMaterializer::NotContainsFromEntries;

        // NotContains requires 2 arguments
        let args = vec![Some(val_ref_int(1))]; // Only 1 arg
        let result = materializer.materialize(&args, &db, NativePredicate::NotContains);

        assert!(result.is_none());
    }

    #[test]
    fn test_not_contains_from_entries_partially_bound() {
        let db = create_test_db();
        let materializer = OperationMaterializer::NotContainsFromEntries;

        let args = vec![None, Some(val_ref_int(0))];
        let result = materializer.materialize(&args, &db, NativePredicate::NotContains);

        assert!(result.is_none());
    }

    #[test]
    fn test_not_contains_from_entries_wrong_predicate() {
        let db = create_test_db();
        let materializer = OperationMaterializer::NotContainsFromEntries;

        // Should not work with Contains predicate
        let args = vec![Some(val_ref_int(1)), Some(val_ref_int(0))];
        let result = materializer.materialize(&args, &db, NativePredicate::Contains);

        assert!(result.is_none());
    }

    // Note: Full array-based Contains/NotContains testing would require
    // creating array Value types, which is complex. The above tests validate
    // the basic argument checking and predicate matching logic.

    #[test]
    fn test_sum_of_all_bound_valid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::SumOf;

        // SumOf(15, 5, 10) where 15 = 5 + 10
        let args = vec![
            Some(val_ref_int(15)),
            Some(val_ref_int(5)),
            Some(val_ref_int(10)),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::SumOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Native(_)));
        assert_eq!(fact.args[0], val_ref_int(15));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_sum_of_all_bound_invalid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::SumOf;

        // SumOf(20, 5, 10) where 20 != 5 + 10
        let args = vec![
            Some(val_ref_int(20)),
            Some(val_ref_int(5)),
            Some(val_ref_int(10)),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::SumOf);

        assert!(result.is_none());
    }

    #[test]
    fn test_sum_of_deduce_result() {
        let db = create_test_db();
        let materializer = OperationMaterializer::SumOf;

        // SumOf(?x, 5, 10) -> x = 15
        let args = vec![None, Some(val_ref_int(5)), Some(val_ref_int(10))];
        let result = materializer.materialize(&args, &db, NativePredicate::SumOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], val_ref_int(15));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_sum_of_deduce_left_operand() {
        let db = create_test_db();
        let materializer = OperationMaterializer::SumOf;

        // SumOf(15, ?y, 10) -> y = 5
        let args = vec![Some(val_ref_int(15)), None, Some(val_ref_int(10))];
        let result = materializer.materialize(&args, &db, NativePredicate::SumOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], val_ref_int(15));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_sum_of_deduce_right_operand() {
        let db = create_test_db();
        let materializer = OperationMaterializer::SumOf;

        // SumOf(15, 5, ?z) -> z = 10
        let args = vec![Some(val_ref_int(15)), Some(val_ref_int(5)), None];
        let result = materializer.materialize(&args, &db, NativePredicate::SumOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], val_ref_int(15));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_sum_of_wrong_argument_count() {
        let db = create_test_db();
        let materializer = OperationMaterializer::SumOf;

        let args = vec![Some(val_ref_int(15)), Some(val_ref_int(5))]; // Only 2 args
        let result = materializer.materialize(&args, &db, NativePredicate::SumOf);

        assert!(result.is_none());
    }

    #[test]
    fn test_product_of_all_bound_valid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::ProductOf;

        // ProductOf(50, 5, 10) where 50 = 5 * 10
        let args = vec![
            Some(val_ref_int(50)),
            Some(val_ref_int(5)),
            Some(val_ref_int(10)),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::ProductOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Native(_)));
        assert_eq!(fact.args[0], val_ref_int(50));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_product_of_deduce_result() {
        let db = create_test_db();
        let materializer = OperationMaterializer::ProductOf;

        // ProductOf(?x, 5, 10) -> x = 50
        let args = vec![None, Some(val_ref_int(5)), Some(val_ref_int(10))];
        let result = materializer.materialize(&args, &db, NativePredicate::ProductOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], val_ref_int(50));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_product_of_deduce_with_division() {
        let db = create_test_db();
        let materializer = OperationMaterializer::ProductOf;

        // ProductOf(50, ?y, 10) -> y = 5
        let args = vec![Some(val_ref_int(50)), None, Some(val_ref_int(10))];
        let result = materializer.materialize(&args, &db, NativePredicate::ProductOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], val_ref_int(50));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_product_of_division_by_zero() {
        let db = create_test_db();
        let materializer = OperationMaterializer::ProductOf;

        // ProductOf(50, ?y, 0) -> division by zero, should fail
        let args = vec![Some(val_ref_int(50)), None, Some(val_ref_int(0))];
        let result = materializer.materialize(&args, &db, NativePredicate::ProductOf);

        assert!(result.is_none());
    }

    #[test]
    fn test_max_of_all_bound_valid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::MaxOf;

        // MaxOf(10, 5, 10) where max(5, 10) = 10
        let args = vec![
            Some(val_ref_int(10)),
            Some(val_ref_int(5)),
            Some(val_ref_int(10)),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::MaxOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Native(_)));
        assert_eq!(fact.args[0], val_ref_int(10));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_max_of_deduce_result() {
        let db = create_test_db();
        let materializer = OperationMaterializer::MaxOf;

        // MaxOf(?x, 5, 10) -> x = 10
        let args = vec![None, Some(val_ref_int(5)), Some(val_ref_int(10))];
        let result = materializer.materialize(&args, &db, NativePredicate::MaxOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], val_ref_int(10));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_max_of_all_bound_invalid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::MaxOf;

        // MaxOf(5, 5, 10) where max(5, 10) = 10, not 5
        let args = vec![
            Some(val_ref_int(5)),
            Some(val_ref_int(5)),
            Some(val_ref_int(10)),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::MaxOf);

        assert!(result.is_none());
    }

    #[test]
    fn test_hash_of_deduce_result() {
        use pod2::middleware::hash_values;

        let db = create_test_db();
        let materializer = OperationMaterializer::HashOf;

        // HashOf(?x, 5, 10) -> x = hash(5, 10)
        let args = vec![None, Some(val_ref_int(5)), Some(val_ref_int(10))];
        let result = materializer.materialize(&args, &db, NativePredicate::HashOf);

        assert!(result.is_some());
        let fact = result.unwrap();

        // Verify the hash was computed correctly
        let expected_hash = hash_values(&[val_int(5), val_int(10)]);
        assert_eq!(fact.args[0], ValueRef::from(expected_hash));
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_hash_of_all_bound_valid() {
        use pod2::middleware::hash_values;

        let db = create_test_db();
        let materializer = OperationMaterializer::HashOf;

        let expected_hash = hash_values(&[val_int(5), val_int(10)]);
        let hash_ref = ValueRef::from(expected_hash);

        // HashOf(hash_val, 5, 10) where hash_val = hash(5, 10)
        let args = vec![
            Some(hash_ref.clone()),
            Some(val_ref_int(5)),
            Some(val_ref_int(10)),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::HashOf);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], hash_ref);
        assert_eq!(fact.args[1], val_ref_int(5));
        assert_eq!(fact.args[2], val_ref_int(10));
    }

    #[test]
    fn test_hash_of_all_bound_invalid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::HashOf;

        // HashOf(wrong_hash, 5, 10) where wrong_hash != hash(5, 10)
        let args = vec![
            Some(val_ref_str("wrong_hash")),
            Some(val_ref_int(5)),
            Some(val_ref_int(10)),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::HashOf);

        assert!(result.is_none());
    }

    #[test]
    fn test_hash_of_cannot_deduce_inputs() {
        let db = create_test_db();
        let materializer = OperationMaterializer::HashOf;

        // HashOf(hash_val, ?y, 10) -> cannot deduce y from hash (not supported)
        let args = vec![Some(val_ref_str("some_hash")), None, Some(val_ref_int(10))];
        let result = materializer.materialize(&args, &db, NativePredicate::HashOf);

        assert!(result.is_none());
    }

    #[test]
    fn test_hash_of_wrong_argument_count() {
        let db = create_test_db();
        let materializer = OperationMaterializer::HashOf;

        let args = vec![Some(val_ref_int(1)), Some(val_ref_int(2))]; // Only 2 args
        let result = materializer.materialize(&args, &db, NativePredicate::HashOf);

        assert!(result.is_none());
    }

    #[test]
    fn test_new_entry_valid_self_reference() {
        use pod2::middleware::SELF;

        let db = create_test_db();
        let materializer = OperationMaterializer::NewEntry;

        // NewEntry with SELF-referencing anchored key
        let self_key = AnchoredKey::new(SELF, Key::new("name".to_string()));
        let args = vec![
            Some(ValueRef::Key(self_key.clone())),
            Some(val_ref_str("Alice")),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::NewEntry));
        assert_eq!(fact.args[0], ValueRef::Key(self_key));
        assert_eq!(fact.args[1], val_ref_str("Alice"));
    }

    #[test]
    fn test_new_entry_non_self_reference() {
        let db = create_test_db();
        let materializer = OperationMaterializer::NewEntry;

        // NewEntry with non-SELF anchored key should fail
        let pod_id = PodId(hash_str("other_pod"));
        let other_key = AnchoredKey::new(pod_id, Key::new("name".to_string()));
        let args = vec![Some(ValueRef::Key(other_key)), Some(val_ref_str("Alice"))];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    #[test]
    fn test_new_entry_literal_first_arg() {
        let db = create_test_db();
        let materializer = OperationMaterializer::NewEntry;

        // NewEntry with literal first arg should fail
        let args = vec![Some(val_ref_str("not_a_key")), Some(val_ref_str("Alice"))];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    #[test]
    fn test_new_entry_partially_bound() {
        use pod2::middleware::SELF;

        let db = create_test_db();
        let materializer = OperationMaterializer::NewEntry;

        // NewEntry with unbound second arg should fail
        let self_key = AnchoredKey::new(SELF, Key::new("name".to_string()));
        let args = vec![Some(ValueRef::Key(self_key)), None];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    #[test]
    fn test_new_entry_wrong_argument_count() {
        let db = create_test_db();
        let materializer = OperationMaterializer::NewEntry;

        let args = vec![Some(val_ref_int(1))]; // Only 1 arg
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    #[test]
    fn test_transitive_equal_from_statements_valid() {
        use pod2::middleware::Statement;

        use crate::db::{IndexablePod, TestPod};

        // Create a database with equality statements A=B, B=C
        let pod_id = PodId(hash_str("test_pod"));
        let key_a = AnchoredKey::new(pod_id, Key::new("a".to_string()));
        let key_b = AnchoredKey::new(pod_id, Key::new("b".to_string()));
        let key_c = AnchoredKey::new(pod_id, Key::new("c".to_string()));

        let test_pod = TestPod {
            id: pod_id,
            statements: vec![
                Statement::Equal(ValueRef::Key(key_a.clone()), ValueRef::Key(key_b.clone())),
                Statement::Equal(ValueRef::Key(key_b.clone()), ValueRef::Key(key_c.clone())),
            ],
        };

        let db = FactDB::build(&[IndexablePod::TestPod(Arc::new(test_pod))]).unwrap();
        let materializer = OperationMaterializer::TransitiveEqualFromStatements;

        // Args for TransitiveEqual: just [key_a, key_c] - should find path through key_b
        let args = vec![
            Some(ValueRef::Key(key_a.clone())),
            Some(ValueRef::Key(key_c.clone())),
        ];

        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], ValueRef::Key(key_a));
        assert_eq!(fact.args[1], ValueRef::Key(key_c));
    }

    #[test]
    fn test_transitive_equal_from_statements_invalid() {
        let db = create_test_db();
        let materializer = OperationMaterializer::TransitiveEqualFromStatements;

        // Test with no connecting path between keys - should fail
        let pod_id = PodId(hash_str("test_pod"));
        let key_a = val_ref_key(pod_id, "a");
        let key_c = val_ref_key(pod_id, "c");

        let args = vec![Some(key_a), Some(key_c)];

        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    // ================================================================================================
    // Tests for Statement Copying Operations
    // ================================================================================================

    #[test]
    fn test_copy_statement_no_data() {
        let db = create_test_db();
        let materializer = OperationMaterializer::CopyStatement;

        // Try to copy from empty database - should not materialize
        let args = vec![Some(val_ref_int(1)), Some(val_ref_int(1))];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    #[test]
    fn test_copy_statement_partially_bound() {
        let db = create_test_db();
        let materializer = OperationMaterializer::CopyStatement;

        let args = vec![Some(val_ref_int(42)), None];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    #[test]
    fn test_copy_statement_unbound_arguments() {
        let db = create_test_db();
        let materializer = OperationMaterializer::CopyStatement;

        let args = vec![None, None];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    // ================================================================================================
    // Tests for Statement Derivation Operations
    // ================================================================================================

    #[test]
    fn test_lt_to_not_equal_no_data() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtToNotEqual;

        // Try to convert from empty database - should not work
        let args = vec![Some(val_ref_int(20)), Some(val_ref_int(25))];
        let result = materializer.materialize(&args, &db, NativePredicate::NotEqual);

        assert!(result.is_none());
    }

    #[test]
    fn test_lt_to_not_equal_partially_bound() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtToNotEqual;

        let args = vec![Some(val_ref_int(20)), None];
        let result = materializer.materialize(&args, &db, NativePredicate::NotEqual);

        assert!(result.is_none());
    }

    #[test]
    fn test_lt_to_not_equal_wrong_target_predicate() {
        let db = create_test_db();
        let materializer = OperationMaterializer::LtToNotEqual;

        // Try to derive Equal from Lt - should not work (wrong target predicate)
        let args = vec![Some(val_ref_int(20)), Some(val_ref_int(25))];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_none());
    }

    // ================================================================================================
    // Tests for Materializers for Predicate Method
    // ================================================================================================

    #[test]
    fn test_materializers_for_equal() {
        let materializers =
            OperationMaterializer::materializers_for_predicate(NativePredicate::Equal);

        // Equal should have: CopyStatement, EqualFromEntries, TransitiveEqualFromStatements
        assert!(materializers.contains(&OperationMaterializer::CopyStatement));
        assert!(materializers.contains(&OperationMaterializer::EqualFromEntries));
        assert!(materializers.contains(&OperationMaterializer::TransitiveEqualFromStatements));

        // Should not have NotEqual materializers
        assert!(!materializers.contains(&OperationMaterializer::NotEqualFromEntries));
        assert!(!materializers.contains(&OperationMaterializer::LtToNotEqual));
    }

    #[test]
    fn test_materializers_for_not_equal() {
        let materializers =
            OperationMaterializer::materializers_for_predicate(NativePredicate::NotEqual);

        // NotEqual should have: CopyStatement, NotEqualFromEntries, LtToNotEqual
        assert!(materializers.contains(&OperationMaterializer::CopyStatement));
        assert!(materializers.contains(&OperationMaterializer::NotEqualFromEntries));
        assert!(materializers.contains(&OperationMaterializer::LtToNotEqual));

        // Should not have Equal-specific materializers
        assert!(!materializers.contains(&OperationMaterializer::EqualFromEntries));
        assert!(!materializers.contains(&OperationMaterializer::TransitiveEqualFromStatements));
    }

    #[test]
    fn test_materializers_for_lt() {
        let materializers = OperationMaterializer::materializers_for_predicate(NativePredicate::Lt);

        assert!(materializers.contains(&OperationMaterializer::CopyStatement));
        assert!(materializers.contains(&OperationMaterializer::LtFromEntries));
        assert!(!materializers.contains(&OperationMaterializer::LtEqFromEntries));
        assert!(!materializers.contains(&OperationMaterializer::EqualFromEntries));
    }

    // ================================================================================================
    // Edge Cases and Error Conditions
    // ================================================================================================

    #[test]
    fn test_invalid_argument_counts() {
        let db = create_test_db();

        // Test binary operation with wrong argument count
        let args = vec![Some(val_ref_int(1))]; // Only 1 arg for binary operation
        let result =
            OperationMaterializer::EqualFromEntries.materialize(&args, &db, NativePredicate::Equal);
        assert!(result.is_none());

        // Test with too many arguments
        let args = vec![
            Some(val_ref_int(1)),
            Some(val_ref_int(2)),
            Some(val_ref_int(3)),
        ];
        let result =
            OperationMaterializer::EqualFromEntries.materialize(&args, &db, NativePredicate::Equal);
        assert!(result.is_none());
    }

    #[test]
    fn test_large_values() {
        let db = create_test_db();

        // Test with large integer values
        let large_val = i64::MAX;
        let args = vec![Some(val_ref_int(large_val)), Some(val_ref_int(large_val))];
        let result =
            OperationMaterializer::EqualFromEntries.materialize(&args, &db, NativePredicate::Equal);
        assert!(result.is_some());
    }

    #[test]
    fn test_unicode_strings() {
        let db = create_test_db();

        // Test with unicode string values
        let unicode_str = "こんにちは🌸";
        let args = vec![
            Some(val_ref_str(unicode_str)),
            Some(val_ref_str(unicode_str)),
        ];
        let result =
            OperationMaterializer::EqualFromEntries.materialize(&args, &db, NativePredicate::Equal);
        assert!(result.is_some());
    }

    #[test]
    fn test_mixed_value_types() {
        let db = create_test_db();

        // Test comparing different value types (should not be equal)
        let args = vec![Some(val_ref_str("42")), Some(val_ref_int(42))];
        let result =
            OperationMaterializer::EqualFromEntries.materialize(&args, &db, NativePredicate::Equal);
        assert!(result.is_none());

        // Should work for NotEqual
        let result = OperationMaterializer::NotEqualFromEntries.materialize(
            &args,
            &db,
            NativePredicate::NotEqual,
        );
        assert!(result.is_some());
    }

    // ================================================================================================
    // Architecture Validation Tests
    // ================================================================================================

    #[test]
    fn test_architectural_separation() {
        let db = create_test_db();

        // Test that value-based operations work without needing database lookups
        let args = vec![Some(val_ref_int(42)), Some(val_ref_int(42))];

        // EqualFromEntries should work (value computation)
        let result =
            OperationMaterializer::EqualFromEntries.materialize(&args, &db, NativePredicate::Equal);
        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], args[0].clone().unwrap());
        assert_eq!(fact.args[1], args[1].clone().unwrap());
    }

    #[test]
    fn test_copy_statement_requires_existing_data() {
        let db = create_test_db();

        // CopyStatement should not materialize without existing statements in database
        let args = vec![Some(val_ref_int(42)), Some(val_ref_int(42))];
        let result =
            OperationMaterializer::CopyStatement.materialize(&args, &db, NativePredicate::Equal);
        assert!(
            result.is_none(),
            "CopyStatement should require existing data in database"
        );
    }

    #[test]
    fn test_derivation_operations_require_source_data() {
        let db = create_test_db();

        // LtToNotEqual should not materialize without existing Lt statements
        let args = vec![Some(val_ref_int(20)), Some(val_ref_int(25))];
        let result =
            OperationMaterializer::LtToNotEqual.materialize(&args, &db, NativePredicate::NotEqual);
        assert!(
            result.is_none(),
            "LtToNotEqual should require existing Lt statements"
        );
    }
}
