use pod2::{
    lang::parse,
    middleware::{Params, Signer},
};
use pod2_new_solver::{
    build_pod_from_answer_top_level_public, custom, edb, Engine, EngineConfigBuilder, OpRegistry,
};
use tracing_subscriber::EnvFilter;

#[test]
fn engine_ethdos_end_to_end() -> Result<(), String> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    use hex::ToHex;
    use pod2::{
        backends::plonky2::{mock::mainpod::MockProver, signer::Signer},
        examples::{attest_eth_friend, custom::eth_dos_batch, MOCK_VD_SET},
        middleware::SecretKey,
    };

    let params = Params {
        max_input_pods_public_statements: 8,
        max_statements: 24,
        max_public_statements: 8,
        ..Default::default()
    };
    let vd_set = &*MOCK_VD_SET;

    let alice = Signer(SecretKey(1u32.into()));
    let bob = Signer(SecretKey(2u32.into()));
    let charlie = Signer(SecretKey(3u32.into()));

    let prover = MockProver {};

    let alice_attestation = attest_eth_friend(&params, &alice, bob.public_key());
    let bob_attestation = attest_eth_friend(&params, &bob, charlie.public_key());

    let batch = eth_dos_batch(&params).unwrap();
    /*
    eth_dos_batch:
        eth_friend(src, dst, private: attestation) = AND(
            SignedBy(attestation, src)
            Contains(attestation, "attestation", dst)
        )

        eth_dos_base(src, dst, distance) = AND(
            Equal(src, dst)
            Equal(distance, 0)
        )

        eth_dos_ind(src, dst, distance, private: shorter_distance, intermed) = AND(
            eth_dos(src, intermed, shorter_distance)
            SumOf(distance, shorter_distance, 1)
            eth_friend(intermed, dst)
        )

        eth_dos(src, dst, distance) = OR(
            eth_dos_base(src, dst, distance)
            eth_dos_ind(src, dst, distance)
        )
    */

    let reg = OpRegistry::default();

    /******************************************************************************
     * Request: eth_dos(alice, alice, Distance)
     ******************************************************************************/
    let req1 = format!(
        r#"
  use _, _, _, eth_dos from 0x{}

  REQUEST(
      eth_dos(PublicKey({}), PublicKey({}), Distance)
  )
  "#,
        batch.id().encode_hex::<String>(),
        alice.public_key(),
        alice.public_key()
    );

    let processed =
        parse(&req1, &params, std::slice::from_ref(&batch)).map_err(|e| e.to_string())?;

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
    custom::register_rules_from_batch(&mut engine.rules, &batch);
    engine.load_processed(&processed);
    engine.run().expect("run ok");

    assert!(!engine.answers.is_empty());

    let pod = build_pod_from_answer_top_level_public(
        &engine.answers[0],
        &params,
        vd_set,
        |b| b.prove(&prover).map_err(|e| e.to_string()),
        &edb,
    )
    .unwrap();

    pod.pod.verify().unwrap();

    /******************************************************************************
     * Request: eth_dos(alice, bob, Distance)
     ******************************************************************************/

    let edb_builder = edb::ImmutableEdbBuilder::new();
    let edb = edb_builder
        .add_signed_dict(alice_attestation)
        .add_main_pod(&pod)
        .build();

    let req2 = format!(
        r#"
    use _, _, _, eth_dos from 0x{}
  
    REQUEST(
        eth_dos(PublicKey({}), PublicKey({}), Distance)
    )
    "#,
        batch.id().encode_hex::<String>(),
        alice.public_key(),
        bob.public_key()
    );

    let processed =
        parse(&req2, &params, std::slice::from_ref(&batch)).map_err(|e| e.to_string())?;

    let mut engine = Engine::with_config(
        &reg,
        &edb,
        EngineConfigBuilder::new()
            .from_params(&params)
            .branch_and_bound_on_ops(true)
            .build(),
    );
    custom::register_rules_from_batch(&mut engine.rules, &batch);
    engine.load_processed(&processed);
    engine.run().expect("run ok");

    assert!(!engine.answers.is_empty());

    let pod = build_pod_from_answer_top_level_public(
        &engine.answers[0],
        &params,
        vd_set,
        |b| b.prove(&prover).map_err(|e| e.to_string()),
        &edb,
    )
    .unwrap();

    pod.pod.verify().unwrap();

    /******************************************************************************
     * Request: eth_dos(alice, charlie, Distance)
     ******************************************************************************/

    let edb_builder = edb::ImmutableEdbBuilder::new();
    let edb = edb_builder
        // Only available data is Bob's attestation of Charlie, and Bob's proof
        // of distance 1 from Alice to Bob.
        // Charlie only needs to know Bob's distance to Alice, and to have a
        // friend attestation from Bob, and doesn't need to know anything about
        // Alice beyond their public key.
        .add_signed_dict(bob_attestation)
        .add_main_pod(&pod)
        .build();

    let req3 = format!(
        r#"
    use _, _, _, eth_dos from 0x{}
  
    REQUEST(
        eth_dos(PublicKey({}), PublicKey({}), Distance)
    )
    "#,
        batch.id().encode_hex::<String>(),
        alice.public_key(),
        charlie.public_key()
    );

    let processed =
        parse(&req3, &params, std::slice::from_ref(&batch)).map_err(|e| e.to_string())?;

    let mut engine = Engine::with_config(
        &reg,
        &edb,
        EngineConfigBuilder::new()
            .from_params(&params)
            .branch_and_bound_on_ops(true)
            .build(),
    );
    custom::register_rules_from_batch(&mut engine.rules, &batch);
    engine.load_processed(&processed);
    engine.run().expect("run ok");

    assert!(!engine.answers.is_empty());

    let pod = build_pod_from_answer_top_level_public(
        &engine.answers[0],
        &params,
        vd_set,
        |b| b.prove(&prover).map_err(|e| e.to_string()),
        &edb,
    )
    .unwrap();

    pod.pod.verify().unwrap();

    println!("{pod}");

    Ok(())
}
