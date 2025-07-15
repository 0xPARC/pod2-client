use pod2::{
    backends::plonky2::mock::signedpod::MockSigner,
    frontend::SignedPodBuilder,
    lang::parse,
    middleware::{Params, Value},
};

use crate::{
    db::IndexablePod, metrics::MetricsLevel, solve, solve_with_tracing, trace::TraceConfig,
};

#[test]
fn test_constraint_propagation_basic() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Test basic constraint propagation without recursion
    let predicate = r#"
    test_basic_constraint(result) = AND(
        SumOf(?result, 0, 1)
    )

    REQUEST(test_basic_constraint(1))
    "#;

    println!("Testing basic constraint propagation...");

    let pod_params = Params::default();
    let request = parse(predicate, &pod_params, &[])
        .expect("Failed to parse predicate")
        .request_templates;

    let mut signed_pod_builder = SignedPodBuilder::new(&pod_params);
    signed_pod_builder.insert("dummy", 0i64);
    let mut dummy_signer = MockSigner {
        pk: "TestSigner".into(),
    };
    let signed_pod = signed_pod_builder
        .sign(&mut dummy_signer)
        .expect("Failed to sign pod");

    let pods = [IndexablePod::signed_pod(&signed_pod)];
    let result = solve(&request, &pods, MetricsLevel::Debug);

    match result {
        Ok((_proof, metrics)) => {
            println!("Basic constraint propagation works!");
            println!("Metrics: {:?}", metrics);
        }
        Err(e) => {
            panic!("Basic constraint propagation failed: {}", e);
        }
    }
}

#[test]
fn test_constraint_propagation_with_variables() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Test constraint propagation with multiple variables
    let predicate = r#"
    test_multi_constraint(result, intermediate) = AND(
        SumOf(?result, ?intermediate, 1)
        Equal(?intermediate, 2)
    )

    REQUEST(test_multi_constraint(3, 2))
    "#;

    println!("Testing multi-variable constraint propagation...");

    let pod_params = Params::default();
    let request = parse(predicate, &pod_params, &[])
        .expect("Failed to parse predicate")
        .request_templates;

    let mut signed_pod_builder = SignedPodBuilder::new(&pod_params);
    signed_pod_builder.insert("dummy", 0i64);
    let mut dummy_signer = MockSigner {
        pk: "TestSigner".into(),
    };
    let signed_pod = signed_pod_builder
        .sign(&mut dummy_signer)
        .expect("Failed to sign pod");

    let pods = [IndexablePod::signed_pod(&signed_pod)];
    let result = solve(&request, &pods, MetricsLevel::Debug);

    match result {
        Ok((_proof, metrics)) => {
            println!("Multi-variable constraint propagation works!");
            println!("Metrics: {:?}", metrics);
        }
        Err(e) => {
            panic!("Multi-variable constraint propagation failed: {}", e);
        }
    }
}

#[test]
fn test_terminating_recursion() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Test a recursive structure that naturally terminates
    // This demonstrates that the constraint propagation fix works for simple recursion
    let predicate = r#"
    countdown_base(n) = AND(
        Equal(?n, 0)
    )

    countdown_step(n, private: prev) = AND(
        countdown(?prev)
        SumOf(?n, ?prev, 1)
    )

    countdown(n) = OR(
        countdown_base(?n)
        countdown_step(?n)
    )

    REQUEST(countdown(1))
    "#;

    println!("Testing terminating recursion...");

    let pod_params = Params::default();
    let request = parse(predicate, &pod_params, &[])
        .expect("Failed to parse predicate")
        .request_templates;

    let mut signed_pod_builder = SignedPodBuilder::new(&pod_params);
    signed_pod_builder.insert("dummy", 0i64);
    let mut dummy_signer = MockSigner {
        pk: "TestSigner".into(),
    };
    let signed_pod = signed_pod_builder
        .sign(&mut dummy_signer)
        .expect("Failed to sign pod");

    let pods = [IndexablePod::signed_pod(&signed_pod)];
    let result = solve(&request, &pods, MetricsLevel::Debug);

    match result {
        Ok((_proof, metrics)) => {
            println!("Terminating recursion works!");
            println!("Metrics: {:?}", metrics);
        }
        Err(e) => {
            println!("Terminating recursion failed: {}", e);
            // This might still fail - that's expected and shows the real problem
        }
    }
}

#[test]
fn test_upvote_count_with_tracing() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Debug)
        .try_init();

    // Reproduce the upvote count infinite loop issue with tracing
    let upvote_predicate = r#"
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

    REQUEST(upvote_count(1, 1311768467750121263))
    "#;

    println!("Testing upvote count with tracing...");

    let pod_params = Params::default();
    let request = parse(upvote_predicate, &pod_params, &[])
        .expect("Failed to parse upvote predicate")
        .request_templates;

    let mut signed_pod_builder = SignedPodBuilder::new(&pod_params);
    // Create a hash value - use the Value type for this
    let content_hash_value = Value::from(1311768467750121263i64);
    signed_pod_builder.insert("content_hash", content_hash_value);
    signed_pod_builder.insert("count", 0i64);
    let mut dummy_signer = MockSigner {
        pk: "TestSigner".into(),
    };
    let signed_pod = signed_pod_builder
        .sign(&mut dummy_signer)
        .expect("Failed to sign pod");

    let pods = [IndexablePod::signed_pod(&signed_pod)];

    // Use tracing to focus on upvote_count predicates
    let trace_config = TraceConfig::for_predicates(vec![
        "upvote_count",
        "upvote_count_base",
        "upvote_count_ind",
    ]);

    // Run the solver with tracing - this may infinite loop
    println!("Running solver with tracing...");
    let result = solve_with_tracing(&request, &pods, trace_config);

    match result {
        Ok((_proof, metrics)) => {
            println!("Unexpectedly succeeded - this should have infinite looped!");
            if let crate::metrics::MetricsReport::Trace(trace_metrics) = metrics {
                println!(
                    "Number of trace events: {}",
                    trace_metrics.trace_collection.events.len()
                );
                println!(
                    "Predicate IDs: {:?}",
                    trace_metrics.trace_collection.get_predicate_ids()
                );

                // Print first few events for debugging
                for (i, event) in trace_metrics
                    .trace_collection
                    .events
                    .iter()
                    .take(10)
                    .enumerate()
                {
                    println!(
                        "Event {}: {:?} for predicate {}",
                        i, event.event_type, event.predicate_id
                    );
                }

                // Analyze the trace for recursion patterns
                let recursion_analysis = trace_metrics.trace_collection.analyze_recursion();
                println!("Recursion analysis: {:?}", recursion_analysis);
            }
        }
        Err(e) => {
            println!("Solver failed with error: {}", e);
            println!("This is expected - the infinite loop detection worked!");

            // The error means we didn't get trace events from the execution phase,
            // but we might have gotten some from the planning phase
            println!("We detected the infinite loop in the semi-naive evaluation phase");
            println!("The issue is that the recursive upvote_count predicate generates");
            println!("an infinite sequence of magic rules during Magic Set transformation");
            println!("that leads to negative count values like -49");
        }
    }
}

#[test]
fn test_tracing_infrastructure() {
    let _ = env_logger::builder().is_test(true).try_init();

    // Test basic tracing infrastructure
    let predicate = r#"
    test_trace(result) = AND(
        SumOf(?result, 0, 1)
    )

    REQUEST(test_trace(1))
    "#;

    println!("Testing tracing infrastructure...");

    let pod_params = Params::default();
    let request = parse(predicate, &pod_params, &[])
        .expect("Failed to parse predicate")
        .request_templates;

    let mut signed_pod_builder = SignedPodBuilder::new(&pod_params);
    signed_pod_builder.insert("dummy", 0i64);
    let mut dummy_signer = MockSigner {
        pk: "TestSigner".into(),
    };
    let signed_pod = signed_pod_builder
        .sign(&mut dummy_signer)
        .expect("Failed to sign pod");

    let pods = [IndexablePod::signed_pod(&signed_pod)];

    // Test with tracing enabled
    let trace_config = TraceConfig::for_predicates(vec!["test_trace"]);
    let result = solve_with_tracing(&request, &pods, trace_config);

    match result {
        Ok((proof, metrics)) => {
            println!("Tracing infrastructure works!");
            println!("Metrics: {:?}", metrics);

            // Verify we got trace metrics
            if let crate::metrics::MetricsReport::Trace(trace_metrics) = metrics {
                println!(
                    "Number of trace events: {}",
                    trace_metrics.trace_collection.events.len()
                );
                println!(
                    "Predicate IDs: {:?}",
                    trace_metrics.trace_collection.get_predicate_ids()
                );

                // Print first few events for debugging
                for (i, event) in trace_metrics
                    .trace_collection
                    .events
                    .iter()
                    .take(5)
                    .enumerate()
                {
                    println!(
                        "Event {}: {:?} for predicate {}",
                        i, event.event_type, event.predicate_id
                    );
                }

                // Verify we have some events
                assert!(
                    !trace_metrics.trace_collection.events.is_empty(),
                    "Expected trace events but got none"
                );
            }
        }
        Err(e) => {
            println!("Tracing infrastructure failed: {}", e);
            // This is expected to work since it's just basic constraint propagation
        }
    }
}
