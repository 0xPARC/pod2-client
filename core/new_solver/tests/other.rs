use pod2::{lang::parse, middleware::Params};
use pod2_new_solver::{
    build_pod_from_answer_top_level_public, custom, edb, Engine, EngineConfigBuilder, OpRegistry,
    ProofDagWithOps,
};
use tracing_subscriber::EnvFilter;

#[test]
fn test_literal_dict() -> Result<(), String> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    use pod2::{backends::plonky2::mock::mainpod::MockProver, examples::MOCK_VD_SET};

    let params = Params::default();
    let vd_set = &*MOCK_VD_SET;
    let prover = MockProver {};

    let reg = OpRegistry::default();

    let req1 = r#"
init_membership(state) = AND(
    DictContains(state, "test", 0)
)

REQUEST(
    init_membership({"admins": [], "members": [], "test": 0})
    SetContains(#["test"], "test")
    ArrayContains([2], 0, 2)
    SetNotContains(#["test"], "not test")
    DictNotContains({"foo": "bar"}, "test")
 )
"#;

    let processed = parse(req1, &params, &[]).map_err(|e| e.to_string())?;

    let edb_builder = edb::ImmutableEdbBuilder::new();
    let edb = edb_builder.build();

    let mut engine = Engine::with_config(
        &reg,
        &edb,
        EngineConfigBuilder::new()
            .from_params(&params)
            .branch_and_bound_on_ops(true)
            .build(),
    );
    custom::register_rules_from_batch(&mut engine.rules, &processed.custom_batch);
    engine.load_processed(&processed);
    engine.run().expect("run ok");

    assert!(!engine.answers.is_empty());

    let dag = ProofDagWithOps::from_store(&engine.answers[0]);

    println!("{}", dag.to_tree_text());

    let pod = build_pod_from_answer_top_level_public(
        &engine.answers[0],
        &params,
        vd_set,
        |b| b.prove(&prover).map_err(|e| e.to_string()),
        &edb,
    )
    .unwrap();

    pod.pod.verify().unwrap();

    Ok(())
}
