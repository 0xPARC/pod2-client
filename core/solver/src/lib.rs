use std::sync::Arc;

use pod2::{backends::plonky2::primitives::ec::schnorr::SecretKey, middleware::StatementTmpl};

use crate::{
    db::{FactDB, IndexablePod},
    engine::semi_naive::SemiNaiveEngine,
    error::SolverError,
    metrics::{
        CounterMetrics, DebugMetrics, MetricsLevel, MetricsReport, MetricsSink, NoOpMetrics,
        TraceMetrics,
    },
    planner::{Planner, QueryPlan},
    proof::Proof,
    semantics::materializer::Materializer,
};

pub mod db;
pub mod debug;
pub mod engine;
pub mod error;
pub mod explainer;
pub mod ir;
pub mod metrics;
pub mod planner;
pub mod pretty_print;
pub mod proof;
pub mod semantics;
pub mod trace;
pub mod vis;

#[derive(Debug, Clone)]
pub struct SolverContext<'a> {
    pods: &'a [IndexablePod],
    keys: &'a [SecretKey],
}

impl<'a> SolverContext<'a> {
    pub fn new(pods: &'a [IndexablePod], keys: &'a [SecretKey]) -> Self {
        Self { pods, keys }
    }
}

/// The main entry point for the solver.
///
/// Takes a proof request, a set of pods containing asserted facts, and runtime
/// parameters, and attempts to find a valid proof. It can be configured to
/// different levels of metrics during execution.
pub fn solve(
    request: &[StatementTmpl],
    context: &SolverContext,
    metrics_level: MetricsLevel,
) -> Result<(Proof, MetricsReport), SolverError> {
    // Common setup logic that is independent of the metrics level.
    let mut db = FactDB::build(context.pods).unwrap();
    for key in context.keys {
        db.add_keypair(key.clone());
    }
    let wrapped_db = Arc::new(db);
    let materializer = Materializer::new(wrapped_db.clone());
    let planner = Planner::new();

    // Dispatch to the appropriate generic implementation based on the desired
    // metrics level. This allows the compiler to monomorphize the engine's
    // execution path and eliminate the overhead of metrics collection when it
    // is not needed.
    match metrics_level {
        MetricsLevel::None => {
            let plan = planner.create_plan(request).unwrap();
            let (proof, _) = run_solve(plan, materializer, NoOpMetrics)?;
            Ok((proof, MetricsReport::None))
        }
        MetricsLevel::Counters => {
            let plan = planner.create_plan(request).unwrap();
            let (proof, metrics) = run_solve(plan, materializer, CounterMetrics::default())?;
            Ok((proof, MetricsReport::Counters(metrics)))
        }
        MetricsLevel::Debug => {
            let plan = planner.create_plan(request).unwrap();
            let (proof, metrics) = run_solve(plan, materializer, DebugMetrics::default())?;
            Ok((proof, MetricsReport::Debug(metrics)))
        }
        MetricsLevel::Trace => {
            let mut metrics = TraceMetrics::default();
            let plan = planner.create_plan_with_metrics(request, &mut metrics)?;
            let (proof, metrics) = run_solve(plan, materializer, metrics)?;
            Ok((proof, MetricsReport::Trace(metrics)))
        }
    }
}

/// The private, generic worker function for the solver.
///
/// This function is monomorphized by the compiler for each concrete `MetricsSink`
/// type, allowing for zero-cost static dispatch of metrics collection.
fn run_solve<M: MetricsSink>(
    plan: QueryPlan,
    materializer: Materializer,
    metrics: M,
) -> Result<(Proof, M), SolverError> {
    let mut engine = SemiNaiveEngine::new(metrics);

    let (all_facts, provenance) = engine.execute(&plan, &materializer)?;
    let proof = engine.reconstruct_proof(&all_facts, &provenance, &materializer)?;

    Ok((proof, engine.into_metrics()))
}

/// Solve with custom trace configuration.
pub fn solve_with_tracing(
    request: &[StatementTmpl],
    pods: &[IndexablePod],
    trace_config: crate::trace::TraceConfig,
) -> Result<(Proof, MetricsReport), SolverError> {
    // Common setup logic that is independent of the metrics level.
    let db = Arc::new(FactDB::build(pods).unwrap());
    let materializer = Materializer::new(db.clone());
    let planner = Planner::new();

    // Use TraceMetrics with the custom configuration
    let mut metrics = TraceMetrics::new(trace_config);
    let plan = planner.create_plan_with_metrics(request, &mut metrics)?;
    let (proof, metrics) = run_solve(plan, materializer, metrics)?;
    Ok((proof, MetricsReport::Trace(metrics)))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use hex::ToHex;
    use pod2::{
        backends::plonky2::{
            mock::mainpod::MockProver, primitives::ec::schnorr::SecretKey, signedpod::Signer,
        },
        examples::{
            attest_eth_friend, custom::eth_dos_batch, zu_kyc_sign_pod_builders, MOCK_VD_SET,
            ZU_KYC_NOW_MINUS_18Y, ZU_KYC_NOW_MINUS_1Y, ZU_KYC_SANCTION_LIST,
        },
        frontend::{MainPodBuilder, OperationArg},
        lang::parse,
        middleware::{containers::Set, NativeOperation, OperationType, Params, Value},
    };

    use super::*;

    #[test]
    fn test_ethdos() {
        let _ = env_logger::builder().is_test(true).try_init();
        let params = Params {
            max_input_pods_public_statements: 8,
            max_statements: 24,
            max_public_statements: 8,
            ..Default::default()
        };

        let alice = Signer(SecretKey::new_rand());
        let bob = Signer(SecretKey::new_rand());
        let charlie = Signer(SecretKey::new_rand());
        let _david = Signer(SecretKey::new_rand());

        let alice_attestation = attest_eth_friend(&params, &alice, bob.public_key());
        let bob_attestation = attest_eth_friend(&params, &bob, charlie.public_key());
        let batch = eth_dos_batch(&params).unwrap();

        let req1 = format!(
            r#"
      use _, _, _, eth_dos from 0x{}

      REQUEST(
          eth_dos({}, {}, ?Distance)
      )
      "#,
            batch.id().encode_hex::<String>(),
            alice.public_key(),
            bob.public_key()
        );

        let request = parse(&req1, &params, std::slice::from_ref(&batch))
            .unwrap()
            .request;

        let context = SolverContext {
            pods: &[IndexablePod::signed_pod(&alice_attestation)],
            keys: &[],
        };

        let (result, _metrics) =
            solve(request.templates(), &context, MetricsLevel::Counters).unwrap();

        let prover = MockProver {};
        #[allow(clippy::borrow_interior_mutable_const)]
        let mut builder = MainPodBuilder::new(&params, &MOCK_VD_SET);

        let (_pod_ids, ops) = result.to_inputs();

        for (op, public) in ops {
            if public {
                builder.pub_op(op).unwrap();
            } else {
                builder.priv_op(op).unwrap();
            }
        }

        builder.add_signed_pod(&alice_attestation);

        let alice_bob_pod = builder.prove(&prover, &params).unwrap();
        let bindings = request.exact_match_pod(&*alice_bob_pod.pod).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings.get("Distance").unwrap(), &Value::from(1));
        println!("{alice_bob_pod}");

        let req2 = format!(
            r#"
      use _, _, _, eth_dos from 0x{}

      REQUEST(
          eth_dos({}, {}, ?Distance)
      )
      "#,
            batch.id().encode_hex::<String>(),
            alice.public_key(),
            charlie.public_key()
        );

        let request = parse(&req2, &params, std::slice::from_ref(&batch))
            .unwrap()
            .request;

        let context = SolverContext {
            pods: &[
                IndexablePod::main_pod(&alice_bob_pod),
                IndexablePod::signed_pod(&bob_attestation),
            ],
            keys: &[],
        };
        let (result, _metrics) =
            solve(request.templates(), &context, MetricsLevel::Counters).unwrap();

        let prover = MockProver {};
        #[allow(clippy::borrow_interior_mutable_const)]
        let mut builder = MainPodBuilder::new(&params, &MOCK_VD_SET);

        let (_pod_ids, ops) = result.to_inputs();
        println!("{result}");

        for (op, public) in ops {
            if public {
                builder.pub_op(op).unwrap();
            } else {
                builder.priv_op(op).unwrap();
            }
        }

        builder.add_signed_pod(&bob_attestation);
        builder.add_recursive_pod(alice_bob_pod);

        let bob_charlie_pod = builder.prove(&prover, &params).unwrap();
        let bindings = request.exact_match_pod(&*bob_charlie_pod.pod).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings.get("Distance").unwrap(), &Value::from(2));
        println!("{bob_charlie_pod}");
    }

    #[test]
    fn test_zukyc() {
        let _ = env_logger::builder().is_test(true).try_init();
        let params = Params::default();

        let const_18y = ZU_KYC_NOW_MINUS_18Y;
        let const_1y = ZU_KYC_NOW_MINUS_1Y;
        let sanctions_values: HashSet<Value> = ZU_KYC_SANCTION_LIST
            .iter()
            .map(|s| Value::from(*s))
            .collect();
        let sanction_set =
            Value::from(Set::new(params.max_depth_mt_containers, sanctions_values).unwrap());

        let (gov_id, pay_stub) = zu_kyc_sign_pod_builders(&params);
        let signer = Signer(SecretKey::new_rand());
        let gov_id = gov_id.sign(&signer).unwrap();

        let signer = Signer(SecretKey::new_rand());
        let pay_stub = pay_stub.sign(&signer).unwrap();

        let zukyc_request = format!(
            r#"
        REQUEST(
            NotContains({sanction_set}, ?gov["idNumber"])
            Lt(?gov["dateOfBirth"], {const_18y})
            Equal(?pay["startDate"], {const_1y})
            Equal(?gov["socialSecurityNumber"], ?pay["socialSecurityNumber"])
            Equal(?self["watermark"], 0)
        )
        "#
        );

        let request = parse(&zukyc_request, &params, &[]).unwrap().request;

        let pods = [
            IndexablePod::signed_pod(&gov_id),
            IndexablePod::signed_pod(&pay_stub),
        ];

        let context = SolverContext {
            pods: &pods,
            keys: &[],
        };

        let (result, _) = solve(request.templates(), &context, MetricsLevel::Counters).unwrap();

        let prover = MockProver {};
        #[allow(clippy::borrow_interior_mutable_const)]
        let mut builder = MainPodBuilder::new(&params, &MOCK_VD_SET);

        let (pod_ids, ops) = result.to_inputs();

        for (op, public) in ops {
            if public {
                println!("public op: {op:?}");
                builder.pub_op(op).unwrap();
            } else {
                builder.priv_op(op).unwrap();
            }
        }

        for pod_id in pod_ids {
            let pod = pods.iter().find(|p| p.id() == pod_id).unwrap();
            if let IndexablePod::SignedPod(pod) = pod {
                builder.add_signed_pod(pod);
            } else {
                panic!("Expected signed pod, got {pod:?}");
            }
        }

        let kyc = builder.prove(&prover, &params).unwrap();

        println!("{kyc}");
    }

    #[test]
    fn test_public_key_of() {
        let params = Params::default();
        let sk = SecretKey::new_rand();
        let pk = sk.public_key();
        let request = parse(
            &format!("REQUEST(PublicKeyOf({}, ?b))", Value::from(pk)),
            &params,
            &[],
        )
        .unwrap();
        let request = request.request;
        let context = SolverContext::new(&[], &[]);
        let solve_result = solve(request.templates(), &context, MetricsLevel::Counters);
        assert!(solve_result.is_err());

        let sks = vec![sk.clone()];
        let context = SolverContext::new(&[], &sks);
        let solve_result = solve(request.templates(), &context, MetricsLevel::Counters);
        assert!(solve_result.is_ok());
        let (proof, _) = solve_result.unwrap();
        let (pod_ids, ops) = proof.to_inputs();
        assert_eq!(pod_ids.len(), 0);
        assert_eq!(ops.len(), 1);
        assert!(matches!(
            ops[0].0 .0,
            OperationType::Native(NativeOperation::PublicKeyOf)
        ));
        assert!(matches!(
            ops[0].0.1.as_slice(),
            [
                OperationArg::Literal(pk_val),
                OperationArg::Literal(sk_val)
            ] if pk_val == &Value::from(pk) && sk_val == &Value::from(sk.clone())
        ));
    }

    #[test]
    fn test_repeated_statements() {
        let _ = env_logger::builder().is_test(true).try_init();
        let params = Params::default();
        let sk = SecretKey::new_rand();
        let pk = Value::from(sk.public_key());
        let request = parse(
            &format!(
                r#"
owned_public_key(pk, pod_id, private: sk) = AND(
    PublicKeyOf(?pk, ?sk)
    Equal(?pod_id, SELF)
)

REQUEST(
    PublicKeyOf({pk}, ?sk)
    owned_public_key({pk}, SELF)
)
            "#
            ),
            &params,
            &[],
        )
        .unwrap();
        let request = request.request;
        let sks = vec![sk.clone()];
        let context = SolverContext::new(&[], &sks);
        let solve_result = solve(request.templates(), &context, MetricsLevel::Counters);
        assert!(solve_result.is_ok());
        let (proof, _) = solve_result.unwrap();
        let (_pod_ids, ops) = proof.to_inputs();

        let mut builder = MainPodBuilder::new(&params, &MOCK_VD_SET);

        for (op, public) in ops {
            if public {
                builder.pub_op(op).unwrap();
            } else {
                builder.priv_op(op).unwrap();
            }
        }

        let prover = MockProver {};
        let pod = builder.prove(&prover, &params).unwrap();

        assert_eq!(pod.public_statements.len(), 3); // Including the _type statement
        println!("{pod}");
    }
}
