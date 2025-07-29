use hex::FromHex;
use num_bigint::BigUint;
use pod2::{
    backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
    frontend::SignedPodBuilder,
    lang::parse,
    middleware::{Hash, Params, Value},
};
use pod2_solver::{SolverContext, db::IndexablePod, metrics::MetricsLevel, solve};

//#[test]
//fn test_full_upvote_count() {
//    let _ = env_logger::builder()
//        .is_test(true)
//        .filter_level(log::LevelFilter::Trace)
//        .try_init();
//    let content_hash =
//        Hash::from_hex("eee73e344ffc120fb787c7650fd9a036362e4d2dc20a3646cc8e9f7112ec4d12").unwrap();
//    let base_params = UpvoteCountBaseParams {
//        content_hash: &content_hash,
//        use_mock_proofs: true,
//    };
//    let base_pod_result = prove_upvote_count_base_with_solver(base_params);
//    assert!(base_pod_result.is_ok());
//
//    let upvote_pod_json =
//        fs::read_to_string("tests/upvote_pod.json").expect("Unable to read upvote_pod.json");
//    let upvote_pod: MainPod = serde_json::from_str(&upvote_pod_json).unwrap();
//
//    let inductive_params = UpvoteCountInductiveParams {
//        content_hash: &content_hash,
//        previous_count: 0,
//        previous_count_pod: &base_pod_result.unwrap(),
//        upvote_verification_pod: &upvote_pod,
//        use_mock_proofs: true,
//    };
//    let inductive_pod_result = prove_upvote_count_inductive_with_solver(inductive_params);
//    assert!(inductive_pod_result.is_ok());
//    let inductive_pod = inductive_pod_result.unwrap();
//    println!("Inductive pod: {inductive_pod}");
//
//    let inductive_params = UpvoteCountInductiveParams {
//        content_hash: &content_hash,
//        previous_count: 1,
//        previous_count_pod: &inductive_pod,
//        upvote_verification_pod: &upvote_pod,
//        use_mock_proofs: true,
//    };
//    let inductive_pod_result = prove_upvote_count_inductive_with_solver(inductive_params);
//    assert!(inductive_pod_result.is_ok());
//    let inductive_pod_two = inductive_pod_result.unwrap();
//    println!("Inductive pod two: {}", inductive_pod_two);
//}

#[test]
fn test_simple_upvote_count() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        .try_init();
    println!("Testing simple upvote count without verification...");

    // Create a simple content hash for testing
    let content_hash =
        Hash::from_hex("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef").unwrap();

    // Create a simple predicate that just does counting without verification
    let simple_predicate = r#"
    upvote_count_base(count, content_hash, private: data_pod) = AND(
        Equal(?count, 0)
        Equal(?data_pod["content_hash"], ?content_hash)
    )

    upvote_count_ind(count, content_hash, private: data_pod, intermed) = AND(
        upvote_count(?intermed, ?content_hash)
        SumOf(?count, ?intermed, 1)
        Equal(?data_pod["content_hash"], ?content_hash)
        Lt(0, ?count)
    )

    upvote_count(count, content_hash) = OR(
        upvote_count_base(?count, ?content_hash)
        upvote_count_ind(?count, ?content_hash)
    )
    "#;

    println!("Simple upvote count predicate: {simple_predicate}");

    // Parse the predicate
    let pod_params = Params::default();
    let _parsed_result =
        parse(simple_predicate, &pod_params, &[]).expect("Failed to parse upvote count predicate");

    // Create the query for base case: upvote_count_base(0, content_hash, private: _)
    let content_hash_value = Value::from(content_hash);
    let mut query = simple_predicate.to_string();
    query.push_str(&format!("REQUEST(upvote_count(0, {content_hash_value}))"));
    println!("Base case query: {query}");

    // Parse the query
    let request = parse(&query, &pod_params, &[])
        .expect("Failed to parse query")
        .request_templates;

    // Create a signed pod with the data
    let mut signed_pod_builder = SignedPodBuilder::new(&pod_params);
    signed_pod_builder.insert("content_hash", content_hash);
    signed_pod_builder.insert("count", 0i64);

    // Sign with a dummy secret key for testing
    let dummy_sk = SecretKey(BigUint::from(12345u64));
    let signed_pod = signed_pod_builder
        .sign(&Signer(dummy_sk))
        .expect("Failed to sign pod");

    // Solve for the base case
    let pods = [IndexablePod::signed_pod(&signed_pod)];
    let context = SolverContext::new(&pods, &[]);
    let (proof, _metrics) =
        solve(&request, &context, MetricsLevel::Debug).expect("Failed to solve base case");

    println!("Base case solved successfully!");
    println!("Proof root nodes: {:?}", proof.root_nodes);

    // Verify the proof (solver proofs don't have a simple verify method, but we can check root nodes)
    assert!(!proof.root_nodes.is_empty());
    println!("✓ Base case proof created successfully!");

    // Now test the inductive case: count = 1
    println!("Testing inductive case: count = 1");

    // Create a second pod for count = 1
    let mut signed_pod_builder2 = SignedPodBuilder::new(&pod_params);
    signed_pod_builder2.insert("content_hash", content_hash);
    signed_pod_builder2.insert("count", 1i64);

    let dummy_sk2 = SecretKey(BigUint::from(12345u64));
    let signed_pod2 = signed_pod_builder2
        .sign(&Signer(dummy_sk2))
        .expect("Failed to sign pod");

    // Query for inductive case: upvote_count_ind(1, content_hash, 0, private: _)
    let mut inductive_query = simple_predicate.to_string();
    inductive_query.push_str(&format!(
        "REQUEST(upvote_count_ind(1, {content_hash_value}))"
    ));
    println!("Inductive query: {inductive_query}");

    // Parse the inductive query
    let inductive_request = parse(&inductive_query, &pod_params, &[])
        .expect("Failed to parse inductive query")
        .request_templates;

    // For inductive case, we need both the base case proof and the new pod
    let pods_inductive = [
        IndexablePod::signed_pod(&signed_pod),
        IndexablePod::signed_pod(&signed_pod2),
    ];
    let context_inductive = SolverContext::new(&pods_inductive, &[]);
    let (proof_inductive, _metrics) =
        solve(&inductive_request, &context_inductive, MetricsLevel::Debug)
            .expect("Failed to solve inductive case");

    println!("Inductive case solved successfully!");
    println!(
        "Inductive proof root nodes: {:?}",
        proof_inductive.root_nodes
    );

    assert!(!proof_inductive.root_nodes.is_empty());
    println!("✓ Inductive case proof created successfully!");
}
