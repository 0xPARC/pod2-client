//! Publish verification MainPod operations

use pod_utils::prover_setup::PodNetProverSetup;
use pod2::{
    frontend::{MainPod, SignedDict},
    lang::parse,
    middleware::{Params, Value, containers::Dictionary},
};
use pod2_new_solver::{
    Engine, EngineConfigBuilder, ImmutableEdbBuilder, OpRegistry,
    build_pod_from_answer_top_level_public,
};

use super::{MainPodError, MainPodResult};
use crate::get_publish_verification_predicate;
// Import the main_pod macro

/// Parameters for publish verification proof generation
pub struct PublishProofParams<'a> {
    pub identity_pod: &'a SignedDict,
    pub document_pod: &'a SignedDict,
    pub use_mock_proofs: bool,
}

/// Generate a publish verification MainPod using the pod2 solver
///
/// This creates a MainPod that cryptographically proves the same properties as
/// prove_publish_verification but uses the automated solver approach instead
/// of manual proof construction.
pub fn prove_publish_verification_with_solver(
    params: PublishProofParams,
) -> MainPodResult<MainPod> {
    // Extract required values from pods
    let username = params
        .identity_pod
        .get("username")
        .ok_or(MainPodError::MissingField {
            pod_type: "Identity",
            field: "username",
        })?;
    let identity_server_pk: Value = params.identity_pod.public_key.into();
    let data = params
        .document_pod
        .get("data")
        .ok_or(MainPodError::MissingField {
            pod_type: "Document",
            field: "data",
        })?
        .clone();

    // Start with the existing predicate definitions and append REQUEST
    let mut query = get_publish_verification_predicate();

    query.push_str(&format!(
        r#"

        REQUEST(
            publish_verified({username}, {data}, {identity_server_pk})
        )
        "#
    ));
    println!("QUERY: {query}");

    // Parse the complete query
    let pod_params = Params::default();
    let request = parse(&query, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {e:?}")))?;

    let edb = ImmutableEdbBuilder::new()
        .add_signed_dict(params.identity_pod.clone())
        .add_signed_dict(params.document_pod.clone())
        .build();

    let reg = OpRegistry::default();
    let config = EngineConfigBuilder::new().from_params(&pod_params).build();
    let mut engine = Engine::with_config(&reg, &edb, config);

    println!("EDB: {}", serde_json::to_string(&edb).unwrap());

    engine.load_processed(&request);
    engine
        .run()
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {e:?}")))?;

    let pod_params = PodNetProverSetup::get_params();
    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    let main_pod = build_pod_from_answer_top_level_public(
        &engine.answers[0],
        &pod_params,
        vd_set,
        |b| b.prove(&*prover).map_err(|e| e.to_string()),
        &edb,
    )
    .map_err(|e| MainPodError::ProofGeneration(format!("Pod build error: {e:?}")))?;

    log::debug!(
        "Inputs: {}\n\n {}",
        serde_json::to_string(&params.document_pod).unwrap(),
        serde_json::to_string(&params.identity_pod).unwrap()
    );

    Ok(main_pod)
}

pub fn verify_publish_verification_with_solver(
    main_pod: &MainPod,
    expected_username: &str,
    expected_data: &Dictionary,
    expected_identity_server_pk: &Value,
) -> MainPodResult<()> {
    // Start with the existing predicate definitions and append REQUEST
    let mut query = get_publish_verification_predicate();

    let username_value = Value::from(expected_username);
    let data_value = Value::from(expected_data.clone());

    query.push_str(&format!(
        r#"

        REQUEST(
            publish_verified({username_value}, {data_value}, {expected_identity_server_pk})
        )
        "#
    ));
    log::debug!("QUERY: {query}");

    // Parse the complete query
    let pod_params = Params::default();
    let request = parse(&query, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {e:?}")))?
        .request;

    request
        .exact_match_pod(&*main_pod.pod)
        .map_err(|e| MainPodError::Verification(format!("Exact match pod error: {e:?}")))?;

    log::debug!("GOT PROOF: {main_pod}");

    Ok(())
}

#[cfg(test)]
mod tests {
    // Add unit tests for publish verification functions

    use pod_utils::prover_setup::PodNetProverSetup;
    use pod2::{lang::parse, middleware::Params};
    use pod2_new_solver::{
        Engine, EngineConfigBuilder, ImmutableEdb, OpRegistry,
        build_pod_from_answer_top_level_public,
    };

    use crate::mainpod::MainPodError;

    #[ignore]
    #[test]
    fn test_publish_verification() {
        tracing_subscriber::fmt::init();
        let serialized_edb = r#"
         {"per_predicate_indexes":"[]","full_dicts":{"40a15a15f1775f9ec57c9cc173acacfc47312d00f59117cf9b13fea4cbf383ac":{"42bd0386a28ebfca8ac534027c8c9aaf1e3f95799eda79f0a99b918f35cd289a":"publish","47b811229a2e01789fa1544e10bd1c420714e4b4baf292ccf9201bacdb90f3af":{"max_depth":6,"kvs":{"authors":{"max_depth":5,"set":["Rob"]},"content_hash":{"Raw":"cde8997260dd04765664a84b93889ea987c4ec14bdb5bd45cbc0d26bede0e30d"},"post_id":{"Int":"-1"},"reply_to":{"Int":"-1"},"tags":{"max_depth":5,"set":[]}}}},"4eb21bc5896a9ecdd2714050c7681174a297923e36b45b9157bfc6c98ad55ebf":{"0687b2a7196cb8263d8270328ecdb6e4bd1fb27ecc691be282fae087b2bf9c68":{"PublicKey":"B3wniJWiwUgfNfj6oKV2beRWuhgBtYFCGSUab6xKCKWkNB4gePLi24m"},"17417d2499f6ade7f3387f402392febcbf2f8f59878ce96cdbe7eaa224200e2a":"Rob","2457fb6a7997ef0687afe36dc26cfb77f057e30b13be3374ec07fb18618df000":"2025-09-08T07:30:51.833205+00:00","410014c23f8d137d972b6bf48a908eeeb095737793331de7ae739295f5c021c7":"strawman-identity-server"}},"full_dict_objs":{"40a15a15f1775f9ec57c9cc173acacfc47312d00f59117cf9b13fea4cbf383ac":{"max_depth":32,"kvs":{"data":{"max_depth":6,"kvs":{"authors":{"max_depth":5,"set":["Rob"]},"content_hash":{"Raw":"cde8997260dd04765664a84b93889ea987c4ec14bdb5bd45cbc0d26bede0e30d"},"post_id":{"Int":"-1"},"reply_to":{"Int":"-1"},"tags":{"max_depth":5,"set":[]}}},"request_type":"publish"}},"4eb21bc5896a9ecdd2714050c7681174a297923e36b45b9157bfc6c98ad55ebf":{"max_depth":32,"kvs":{"identity_server_id":"strawman-identity-server","issued_at":"2025-09-08T07:30:51.833205+00:00","user_public_key":{"PublicKey":"B3wniJWiwUgfNfj6oKV2beRWuhgBtYFCGSUab6xKCKWkNB4gePLi24m"},"username":"Rob"}}},"signed_dicts":{"40a15a15f1775f9ec57c9cc173acacfc47312d00f59117cf9b13fea4cbf383ac":{"dict":{"max_depth":32,"kvs":{"data":{"max_depth":6,"kvs":{"authors":{"max_depth":5,"set":["Rob"]},"content_hash":{"Raw":"cde8997260dd04765664a84b93889ea987c4ec14bdb5bd45cbc0d26bede0e30d"},"post_id":{"Int":"-1"},"reply_to":{"Int":"-1"},"tags":{"max_depth":5,"set":[]}}},"request_type":"publish"}},"public_key":"B3wniJWiwUgfNfj6oKV2beRWuhgBtYFCGSUab6xKCKWkNB4gePLi24m","signature":"hw8SA0zaj4ITkf0eOb+3ks9MVfU3iH3Mttn+NKE3HLcYwIwmK870FI6k7/0PdLdcLWsXmvS/hqUkLdIQqDi2Y435d22hCSbKpTUpMtLirNc="},"4eb21bc5896a9ecdd2714050c7681174a297923e36b45b9157bfc6c98ad55ebf":{"dict":{"max_depth":32,"kvs":{"identity_server_id":"strawman-identity-server","issued_at":"2025-09-08T07:30:51.833205+00:00","user_public_key":{"PublicKey":"B3wniJWiwUgfNfj6oKV2beRWuhgBtYFCGSUab6xKCKWkNB4gePLi24m"},"username":"Rob"}},"public_key":"81XmHMoxDXka5UPoTpy2VXo77se4mSSPzbBaXFBMnebhMu5GetHRtwi","signature":"8pWhKvziTPDdomccWF200HIgOy5ZlEjepYD13XsR+TwzllurAauHJ+6wZwtDF2P4tyrJrDvLTLECzjOdnGDcWHW1sfdOdbm4YBKMWtg7jHg="}},"pods":{},"keypairs":{}}
        "#;
        let edb = serde_json::from_str::<ImmutableEdb>(serialized_edb).unwrap();

        let query = r#"
        identity_verified(username, identity_pod) = AND(
            Equal(identity_pod["username"], username)
        )

        publish_verified(username, data, identity_server_pk, private: identity_pod, document_pod) = AND(
            identity_verified(username, identity_pod)
            Equal(document_pod["request_type"], "publish")
            Equal(document_pod["data"], data)
            SignedBy(document_pod, identity_pod["user_public_key"])
            SignedBy(identity_pod, identity_server_pk)
        )
        
        REQUEST(
            publish_verified("Rob", { "authors": #["Rob"], "content_hash": Raw(0xcde8997260dd04765664a84b93889ea987c4ec14bdb5bd45cbc0d26bede0e30d), "post_id": -1, "reply_to": -1, "tags": #[] }, PublicKey(81XmHMoxDXka5UPoTpy2VXo77se4mSSPzbBaXFBMnebhMu5GetHRtwi))
        )"#;

        let pod_params = Params::default();
        let request = parse(query, &pod_params, &[]).unwrap();

        let reg = OpRegistry::default();
        let config = EngineConfigBuilder::new().from_params(&pod_params).build();
        let mut engine = Engine::with_config(&reg, &edb, config);
        engine.load_processed(&request);
        engine.run().unwrap();

        // println!("GOT ANSWER: {:?}", &engine.answers[0]);

        let (vd_set, prover) = PodNetProverSetup::create_prover_setup(true)
            .map_err(MainPodError::ProofGeneration)
            .unwrap();

        let main_pod = build_pod_from_answer_top_level_public(
            &engine.answers[0],
            &pod_params,
            vd_set,
            |b| b.prove(&*prover).map_err(|e| e.to_string()),
            &edb,
        )
        .unwrap();

        println!("GOT MAINPOD: {main_pod}");
        main_pod.pod.verify().unwrap();
    }
}
