//! Unit tests for OperationMaterializer system

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use pod2::{
        backends::plonky2::primitives::ec::schnorr::SecretKey,
        middleware::{
            containers::Dictionary, hash_str, AnchoredKey, Key, NativeOperation, NativePredicate,
            Params, PodId, Value, ValueRef, SELF,
        },
    };

    use crate::{
        db::FactDB,
        engine::semi_naive::{Fact, FactSource},
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
    fn test_contains_from_entries_unbound_value() {
        let db = create_test_db();
        let materializer = OperationMaterializer::ContainsFromEntries;
        let params = Params::default();
        let dict = Dictionary::new(
            params.max_depth_mt_containers,
            HashMap::from([(Key::from("num"), val_int(42))]),
        )
        .unwrap();
        let dict_value_ref = ValueRef::from(Value::new(dict.into()));

        // Partially bound args - "value" arg is unbound, but container and key are bound
        let args = vec![Some(dict_value_ref.clone()), Some(val_ref_str("num")), None];
        let result = materializer.materialize(&args, &db, NativePredicate::Contains);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert_eq!(fact.args[0], dict_value_ref);
        assert_eq!(fact.args[1], val_ref_str("num"));
        assert_eq!(fact.args[2], val_ref_int(42));
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

    // ================================================================================================
    // Tests for Arithmetic/Computation Operations
    // ================================================================================================

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

    // ================================================================================================
    // Tests for NewEntry Operations
    // ================================================================================================

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

        let self_key = AnchoredKey::new(SELF, Key::new("name".to_string()));

        // NewEntry with literal first arg should fail
        let args = vec![
            Some(val_ref_str("not_a_key")),
            Some(ValueRef::Key(self_key)),
        ];
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

    // ================================================================================================
    // Tests for TransitiveEqualFromStatements Operations
    // ================================================================================================

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
    fn test_copy_statement_success() {
        use pod2::middleware::Statement;

        use crate::db::{IndexablePod, TestPod};

        // Create a database with an equality statement A=B
        let pod_id = PodId(hash_str("test_pod"));
        let key_a = AnchoredKey::new(pod_id, Key::new("a".to_string()));
        let key_b = AnchoredKey::new(pod_id, Key::new("b".to_string()));

        let test_pod = TestPod {
            id: pod_id,
            statements: vec![Statement::Equal(
                ValueRef::Key(key_a.clone()),
                ValueRef::Key(key_b.clone()),
            )],
        };

        let db = FactDB::build(&[IndexablePod::TestPod(Arc::new(test_pod))]).unwrap();
        let materializer = OperationMaterializer::CopyStatement;

        // Copy the equality statement from the database
        let args = vec![
            Some(ValueRef::Key(key_a.clone())),
            Some(ValueRef::Key(key_b.clone())),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::Equal);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Copy));
        assert_eq!(fact.args[0], ValueRef::Key(key_a));
        assert_eq!(fact.args[1], ValueRef::Key(key_b));
    }

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
    fn test_lt_to_not_equal_success() {
        use pod2::middleware::Statement;

        use crate::db::{IndexablePod, TestPod};

        // Create a database with a Lt statement A < B
        let pod_id = PodId(hash_str("test_pod"));
        let key_a = AnchoredKey::new(pod_id, Key::new("a".to_string()));
        let key_b = AnchoredKey::new(pod_id, Key::new("b".to_string()));

        let test_pod = TestPod {
            id: pod_id,
            statements: vec![Statement::Lt(
                ValueRef::Key(key_a.clone()),
                ValueRef::Key(key_b.clone()),
            )],
        };

        let db = FactDB::build(&[IndexablePod::TestPod(Arc::new(test_pod))]).unwrap();
        let materializer = OperationMaterializer::LtToNotEqual;

        // Derive NotEqual from Lt statement - if A < B, then A != B
        let args = vec![
            Some(ValueRef::Key(key_a.clone())),
            Some(ValueRef::Key(key_b.clone())),
        ];
        let result = materializer.materialize(&args, &db, NativePredicate::NotEqual);

        assert!(result.is_some());
        let fact = result.unwrap();
        assert!(matches!(fact.source, FactSource::Native(_)));
        assert_eq!(fact.args[0], ValueRef::Key(key_a));
        assert_eq!(fact.args[1], ValueRef::Key(key_b));
    }

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
    // Tests for PublicKeyOf Operations
    // ================================================================================================

    #[test]
    fn test_public_key_of_success() {
        let mut db = create_test_db();
        let sk = SecretKey::new_rand();
        db.add_keypair(sk.clone());
        let materializer = OperationMaterializer::PublicKeyOf;

        let args = vec![
            Some(ValueRef::from(sk.public_key())),
            Some(ValueRef::from(sk.clone())),
        ];

        let relation = materializer.materialize_relation(&args, &db, NativePredicate::PublicKeyOf);
        assert_eq!(relation.len(), 1);
        let fact = relation.iter().next().unwrap();
        assert_eq!(fact.args[0], ValueRef::from(sk.public_key()));
        assert_eq!(fact.args[1], ValueRef::from(sk));
    }

    #[test]
    fn test_public_key_of_with_unbound_secret_key() {
        let mut db = create_test_db();
        let sk = SecretKey::new_rand();
        db.add_keypair(sk.clone());
        let materializer = OperationMaterializer::PublicKeyOf;

        let args = vec![Some(ValueRef::from(sk.public_key())), None];

        let relation = materializer.materialize_relation(&args, &db, NativePredicate::PublicKeyOf);
        assert_eq!(relation.len(), 1);
        let fact = relation.iter().next().unwrap();
        assert_eq!(fact.args[0], ValueRef::from(sk.public_key()));
        assert_eq!(fact.args[1], ValueRef::from(sk));
    }

    #[test]
    fn test_public_key_of_deduce_pk() {
        let mut db = create_test_db();
        let sk = SecretKey::new_rand();
        db.add_keypair(sk.clone());
        let materializer = OperationMaterializer::PublicKeyOf;

        let args = vec![None, Some(ValueRef::from(sk.clone()))];

        let relation = materializer.materialize_relation(&args, &db, NativePredicate::PublicKeyOf);
        assert_eq!(relation.len(), 1);
        let fact = relation.iter().next().unwrap();
        assert_eq!(fact.args[0], ValueRef::from(sk.public_key()));
        assert_eq!(fact.args[1], ValueRef::from(sk));
    }

    #[test]
    fn test_public_key_of_unbound_produces_all_pairs() {
        let mut db = create_test_db();
        let sk1 = SecretKey::new_rand();
        let sk2 = SecretKey::new_rand();
        db.add_keypair(sk1.clone());
        db.add_keypair(sk2.clone());
        let materializer = OperationMaterializer::PublicKeyOf;

        let args = vec![None, None];

        let relation = materializer.materialize_relation(&args, &db, NativePredicate::PublicKeyOf);
        assert_eq!(relation.len(), 2);

        let fact1 = Fact {
            source: FactSource::Native(NativeOperation::PublicKeyOf),
            args: vec![
                ValueRef::from(sk1.public_key()),
                ValueRef::from(sk1.clone()),
            ],
        };
        let fact2 = Fact {
            source: FactSource::Native(NativeOperation::PublicKeyOf),
            args: vec![
                ValueRef::from(sk2.public_key()),
                ValueRef::from(sk2.clone()),
            ],
        };

        assert!(relation.contains(&fact1));
        assert!(relation.contains(&fact2));
    }

    #[test]
    fn test_public_key_of_with_mismatched_public_key() {
        let db = create_test_db();
        let materializer = OperationMaterializer::PublicKeyOf;

        let args = vec![
            Some(ValueRef::from(SecretKey::new_rand().public_key())),
            Some(ValueRef::from(SecretKey::new_rand())),
        ];

        let relation = materializer.materialize_relation(&args, &db, NativePredicate::PublicKeyOf);
        assert!(relation.is_empty());
    }

    #[test]
    fn test_public_key_of_with_unknown_public_key() {
        let db = create_test_db();
        let materializer = OperationMaterializer::PublicKeyOf;
        let args = vec![
            // Public key is not in the database
            Some(ValueRef::from(SecretKey::new_rand().public_key())),
            None,
        ];
        let relation = materializer.materialize_relation(&args, &db, NativePredicate::PublicKeyOf);
        assert!(relation.is_empty());
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
}
