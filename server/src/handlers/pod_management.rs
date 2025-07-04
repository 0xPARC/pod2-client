use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use hex::ToHex;
use pod2::{
    backends::plonky2::mock::signedpod::MockSigner,
    frontend::{MainPod, SerializedMainPod, SerializedSignedPod, SignedPod, SignedPodBuilder},
    middleware::{hash_str, Hash, Value as PodValue},
};
use serde::Deserialize;

use super::AppError;
use crate::{
    api_types::{PodData, PodInfo},
    db::{store, Db},
};

// Request body for the /api/pods/sign endpoint
#[derive(Deserialize)]
pub struct SignRequest {
    private_key: String,
    entries: HashMap<String, PodValue>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImportPodRequest {
    pod_type: String,
    data: serde_json::Value,
    label: Option<String>,
}

// Handler for POST /api/pods/:space_id
pub async fn import_pod_to_space(
    State(db): State<Arc<Db>>,
    Path(space_id): Path<String>,
    Json(payload): Json<ImportPodRequest>,
) -> Result<impl IntoResponse, AppError> {
    if !store::space_exists(&db, &space_id).await? {
        return Err(AppError::NotFound(format!(
            "Space with id '{}' not found",
            space_id
        )));
    }

    let created_at = Utc::now().to_rfc3339();

    // The handler is now responsible for deserializing the incoming raw data
    // into the correct, validated `PodData` enum.
    let pod_data_enum = match payload.pod_type.as_str() {
        "signed" => {
            let helper: SerializedSignedPod = serde_json::from_value(payload.data)
                .context("Failed to deserialize data into SerializedSignedPod")?;
            PodData::Signed(helper)
        }
        "main" => {
            let helper: SerializedMainPod = serde_json::from_value(payload.data)
                .context("Failed to deserialize data into SerializedMainPod")?;
            PodData::Main(helper)
        }
        _ => {
            // This is a new error variant I need to add to AppError
            return Err(AppError::BadRequest(format!(
                "Invalid pod type '{}'",
                payload.pod_type
            )));
        }
    };

    // It also determines the pod ID from the deserialized data.
    let pod_id_obj = match &pod_data_enum {
        PodData::Signed(signed_pod_helper) => SignedPod::try_from(signed_pod_helper.clone())
            .map_err(|e| AppError::BadRequest(format!("Invalid signed pod data: {}", e)))?
            .id(),
        PodData::Main(main_pod_helper) => MainPod::try_from(main_pod_helper.clone())
            .map_err(|e| AppError::BadRequest(format!("Invalid main pod data: {}", e)))?
            .id(),
    };
    let pod_id_string = pod_id_obj.0.encode_hex::<String>();

    // Then it calls the generic store function.
    store::import_pod(
        &db,
        &pod_id_string,
        &payload.pod_type,
        &pod_data_enum,
        payload.label.as_deref(),
        &space_id,
    )
    .await?;

    // Finally, it constructs the response object.
    let created_pod_info = PodInfo {
        id: pod_id_string,
        pod_type: payload.pod_type,
        data: pod_data_enum,
        label: payload.label,
        created_at,
        space: space_id,
    };

    Ok((StatusCode::CREATED, Json(created_pod_info)))
}

// Handler for DELETE /api/pods/:space_id/:pod_id
pub async fn delete_pod_from_space(
    State(db): State<Arc<Db>>,
    Path((space_id, pod_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let rows_deleted = store::delete_pod(&db, &space_id, &pod_id).await?;

    if rows_deleted == 0 {
        Err(AppError::NotFound(format!(
            "Pod with id '{}' not found in space '{}'",
            pod_id, space_id
        )))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

// Handler for GET /api/pods/:space
pub async fn list_pods_in_space(
    State(db): State<Arc<Db>>,
    Path(space): Path<String>,
) -> Result<Json<Vec<PodInfo>>, AppError> {
    let pods = store::list_pods(&db, &space).await?;
    Ok(Json(pods))
}

// Handler for GET /api/pods/:space/:id
pub async fn get_pod_by_id(
    State(db): State<Arc<Db>>,
    Path((space, id)): Path<(String, String)>,
) -> Result<Json<PodInfo>, AppError> {
    let pod_info = store::get_pod(&db, &space, &id).await?;

    match pod_info {
        Some(pod) => Ok(Json(pod)),
        None => Err(AppError::NotFound(format!(
            "Pod with id '{}' not found in space '{}'",
            id, space
        ))),
    }
}

// Handler for POST /api/pods/sign
pub async fn sign_pod(Json(payload): Json<SignRequest>) -> Result<Json<SignedPod>, AppError> {
    log::debug!("Received sign request: {:?}", "payload hidden");

    let mut signer = MockSigner {
        pk: payload.private_key,
    };
    log::debug!("Created signer for pk: {}", signer.pk);

    let params = pod2::middleware::Params::default();
    let mut builder = SignedPodBuilder::new(&params);
    log::debug!("Created SignedPodBuilder");

    for (key, value) in payload.entries {
        log::trace!("Inserting entry: key='{}', value=...", key);
        builder.insert(&key, value);
    }
    log::debug!("Inserted all entries into builder");

    let signed_pod = builder
        .sign(&mut signer)
        .context("Failed to sign the POD")?;
    log::debug!("Successfully signed POD with id: {}", signed_pod.id().0);

    Ok(Json(signed_pod))
}

// Handler for POST /api/hash
pub async fn hash_string(body: String) -> Result<Json<Hash>, AppError> {
    log::debug!("Received hash request for string: {:?}", body);
    let hash_result = hash_str(&body);
    log::debug!("Computed hash: {}", hash_result);
    Ok(Json(hash_result))
}

#[cfg(test)]
mod tests {
    use axum_test::TestServer;
    use hex::ToHex;
    use pod2::{
        backends::plonky2::mock::{mainpod::MockProver, signedpod::MockSigner},
        frontend::{MainPodBuilder, SignedPod, SignedPodBuilder},
        middleware::{self, hash_str, Key, Params as PodParams, PodId, Value as PodValue},
        op,
    };
    use serde_json::{json, Value};
    use tracing_subscriber::prelude::*;

    use super::*; // Imports PodInfo, SignRequest etc. and handlers
    use crate::{
        db::{self},
        handlers::playground::MOCK_VD_SET,
        routes::create_router,
    };

    // Helper to insert a test space
    async fn insert_test_space(db: &Db, id: &str) {
        let id_owned = id.to_string();
        store::create_space(db, &id_owned)
            .await
            .expect("Failed to create test space");
    }

    // Helper to insert a test pod (contract for data_payload changes)
    async fn insert_test_pod(
        db: &Db,
        id: &str,       // This MUST be the hex string of the PodId embedded in data_payload
        pod_type: &str, // Should be "main" or "signed"
        data_payload: &Value, // MUST be a valid SerializedSignedPod or SerializedMainPod as JSON
        label: Option<&str>,
        space: &str,
    ) {
        let id_owned = id.to_string();
        let pod_type_owned = pod_type.to_string();
        let label_owned = label.map(|s| s.to_string());
        let space_owned = space.to_string();

        let pod_data_enum_for_test = match pod_type {
            "signed" => {
                let helper = serde_json::from_value(data_payload.clone())
                    .expect("Test data failed to deserialize to SerializedSignedPod");
                PodData::Signed(helper)
            }
            "main" => {
                // For "main" pods, data_payload is the full MainPodHelper structure
                let helper: SerializedMainPod = serde_json::from_value(data_payload.clone())
                    .expect("Test data failed to deserialize to SerializedMainPod");
                PodData::Main(helper)
            }
            _ => panic!(
                "Unsupported pod_type '{}' in test setup. Must be \"main\" or \"signed\".",
                pod_type
            ),
        };

        store::import_pod(
            db,
            &id_owned,
            &pod_type_owned,
            &pod_data_enum_for_test,
            label_owned.as_deref(),
            &space_owned,
        )
        .await
        .expect("Failed to import test pod");
    }

    // Helper to deserialize response
    async fn response_to_pods(response: axum_test::TestResponse) -> Vec<PodInfo> {
        response.json()
    }

    // Test server setup specific to these tests
    pub async fn create_test_server_with_db() -> (TestServer, Arc<Db>) {
        let db = Arc::new(
            db::Db::new(None, &db::MIGRATIONS)
                .await
                .expect("Failed to init db for test"),
        );
        let router = create_router(db.clone());
        (TestServer::new(router).unwrap(), db)
    }

    // Helper function to create sample SerializedSignedPod data as serde_json::Value
    fn create_sample_signed_pod_data(
        params: &PodParams,
        unique_id_str: &str,
        entries: Vec<(&str, PodValue)>,
    ) -> (String, Value) {
        let mut signed_builder = SignedPodBuilder::new(params);
        for (key, value) in entries {
            signed_builder.insert(key, value);
        }
        // Use part of unique_id_str for signer to ensure some variation if needed, though id() is content-based
        let mut signer = MockSigner {
            pk: format!("test_signer_{}", unique_id_str),
        };
        let signed_pod = signed_builder
            .sign(&mut signer)
            .expect("Failed to sign sample pod");

        let pod_id_str = signed_pod.id().0.encode_hex();
        let data_payload_value = serde_json::to_value(&signed_pod)
            .expect("Failed to serialize SignedPod for sample data");
        (pod_id_str, data_payload_value)
    }

    // Helper function to create sample SerializedMainPod data as serde_json::Value
    fn create_sample_main_pod_data(params: &PodParams) -> (String, Value) {
        #[allow(clippy::borrow_interior_mutable_const)]
        let mut main_builder = MainPodBuilder::new(params, &MOCK_VD_SET);
        // Add a simple operation: eq for a new entry
        main_builder
            .priv_op(op!(
                new_entry,
                "test",
                middleware::Value::from("sample_value")
            ))
            .expect("Failed to add new_entry op to sample main pod builder");

        let prover = MockProver {};
        let main_pod = main_builder
            .prove(&prover, params)
            .expect("Failed to prove sample main pod");

        let pod_id_str = main_pod.id().0.encode_hex();
        let data_payload_value =
            serde_json::to_value(&main_pod).expect("Failed to serialize MainPod for sample data");
        (pod_id_str, data_payload_value)
    }

    #[tokio::test]
    async fn test_list_pods_in_space() {
        let (server, db) = create_test_server_with_db().await;

        // Create spaces before inserting pods
        insert_test_space(&db, "space1").await;
        insert_test_space(&db, "space2").await;

        let test_params = PodParams::default();

        // Create and insert a MainPod using helper
        let (main_pod1_id_str, main_pod1_data_payload) = create_sample_main_pod_data(&test_params);
        insert_test_pod(
            &db,
            &main_pod1_id_str,
            "main",
            &main_pod1_data_payload,
            Some("label1"),
            "space1",
        )
        .await;

        // Create and insert a SignedPod using helper
        let (signed_pod1_id_str, signed_pod1_data_payload) = create_sample_signed_pod_data(
            &test_params,
            "list_signed1",
            vec![("value_signed", PodValue::from(2))],
        );
        insert_test_pod(
            &db,
            &signed_pod1_id_str,
            "signed",
            &signed_pod1_data_payload,
            None,
            "space1",
        )
        .await;

        // Create and insert another MainPod for space2 using helper
        let (main_pod2_id_str, main_pod2_data_payload) = create_sample_main_pod_data(&test_params);
        insert_test_pod(
            &db,
            &main_pod2_id_str,
            "main",
            &main_pod2_data_payload,
            Some("label3"),
            "space2",
        )
        .await;

        let response = server.get("/api/pods/space1").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        let pods = response_to_pods(response).await;

        assert_eq!(pods.len(), 2);

        let expected_serialized_main_pod1: SerializedMainPod =
            serde_json::from_value(main_pod1_data_payload.clone()).unwrap();
        let expected_pod_data1 = PodData::Main(expected_serialized_main_pod1);
        assert!(pods.iter().any(|p| p.id == main_pod1_id_str
            && p.pod_type == "main"
            && p.data == expected_pod_data1
            && p.label == Some("label1".to_string())
            && p.space == "space1"));

        let expected_serialized_signed_pod1: SerializedSignedPod =
            serde_json::from_value(signed_pod1_data_payload.clone()).unwrap();
        let expected_pod_data2 = PodData::Signed(expected_serialized_signed_pod1);
        assert!(pods.iter().any(|p| p.id == signed_pod1_id_str
            && p.pod_type == "signed"
            && p.data == expected_pod_data2
            && p.space == "space1"));

        let response_space2 = server.get("/api/pods/space2").await;
        assert_eq!(response_space2.status_code(), StatusCode::OK);
        let pods_space2 = response_to_pods(response_space2).await;

        assert_eq!(pods_space2.len(), 1);
        let p2 = &pods_space2[0];
        assert_eq!(p2.id, main_pod2_id_str);
        assert_eq!(p2.pod_type, "main");
        let expected_serialized_main_pod2: SerializedMainPod =
            serde_json::from_value(main_pod2_data_payload.clone()).unwrap();
        assert_eq!(p2.data, PodData::Main(expected_serialized_main_pod2));
        assert_eq!(p2.label, Some("label3".to_string()));
    }

    #[tokio::test]
    async fn test_get_pod_by_id_success() {
        let (server, db) = create_test_server_with_db().await;
        let space_name = "get_test_space";
        insert_test_space(&db, space_name).await;

        let test_params = PodParams::default();
        let (main_pod_id_str_for_get, main_pod_data_payload_for_get) =
            create_sample_main_pod_data(&test_params);

        // Fetch created_at after insertion for reliable comparison
        insert_test_pod(
            &db,
            &main_pod_id_str_for_get,
            "main",
            &main_pod_data_payload_for_get,
            Some("test_label_get"),
            space_name,
        )
        .await;

        let response = server
            .get(&format!(
                "/api/pods/{}/{}",
                space_name, main_pod_id_str_for_get
            ))
            .await;
        assert_eq!(response.status_code(), StatusCode::OK);
        let pod: PodInfo = response.json();

        assert_eq!(pod.id, main_pod_id_str_for_get);
        assert_eq!(pod.pod_type, "main");
        assert_eq!(pod.space, space_name);
        let expected_serialized_main_pod: SerializedMainPod =
            serde_json::from_value(main_pod_data_payload_for_get.clone()).unwrap();
        assert_eq!(pod.data, PodData::Main(expected_serialized_main_pod));
        assert_eq!(pod.label, Some("test_label_get".to_string()));
    }

    #[tokio::test]
    async fn test_get_pod_by_id_not_found_in_space() {
        let (server, db) = create_test_server_with_db().await;

        let space_name_correct = "other_space_for_pod";
        let space_name_request = "test_space_for_request";

        insert_test_space(&db, space_name_correct).await;
        insert_test_space(&db, space_name_request).await; // Ensure requesting space exists

        let test_params = PodParams::default();
        let (pod_id_str, pod_data) = create_sample_main_pod_data(&test_params);

        insert_test_pod(
            &db,
            &pod_id_str,
            "main",
            &pod_data,
            None,
            space_name_correct, // Pod is in 'other_space_for_pod'
        )
        .await;

        let response = server
            .get(&format!("/api/pods/{}/{}", space_name_request, pod_id_str))
            .await;
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
        let body = response.text();
        assert!(body.contains(
            format!(
                "Pod with id '{}' not found in space '{}'",
                pod_id_str, space_name_request
            )
            .as_str()
        ));
    }

    #[tokio::test]
    async fn test_get_pod_by_id_not_found_id() {
        let (server, db) = create_test_server_with_db().await;
        let space_name = "test_space_for_id_not_found";
        insert_test_space(&db, space_name).await;

        let test_params = PodParams::default();
        let (existing_pod_id_str, existing_pod_data) = create_sample_main_pod_data(&test_params);

        insert_test_pod(
            &db,
            &existing_pod_id_str,
            "main",
            &existing_pod_data,
            None,
            space_name,
        )
        .await;

        let non_existent_id_str: String = PodId(hash_str("non_existent_pod_id_string_for_test"))
            .0
            .encode_hex(); // Create a plausible but non-existent ID
        let response = server
            .get(&format!("/api/pods/{}/{}", space_name, non_existent_id_str))
            .await;
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
        let body = response.text();
        assert!(body.contains(
            format!(
                "Pod with id '{}' not found in space '{}'",
                non_existent_id_str, space_name
            )
            .as_str()
        ));
    }

    #[tokio::test]
    async fn test_sign_pod_success() {
        let _guard = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .set_default();

        let (server, _pool) = create_test_server_with_db().await;

        let private_key = "a".repeat(64);
        let request_payload = json!({
            "private_key": private_key,
            "entries": {
                "name": "Alice",
                "age": { "Int": "30" },
                "city": "Metropolis",
                "verified": true,
                "tags": { "max_depth": 32, "array": [
                    "a",
                    "b",
                    { "Int": "123" }
                ]}
            }
        });

        let response = server.post("/api/pods/sign").json(&request_payload).await;
        assert_eq!(response.status_code(), StatusCode::OK);
        let response_pod: SignedPod = response.json();

        assert!(response_pod.verify().is_ok());
        assert_eq!(response_pod.kvs().len(), 7);
        assert!(response_pod.kvs().contains_key(&Key::from("name")));
        assert_eq!(
            response_pod.kvs().get(&Key::from("name")),
            Some(&PodValue::from("Alice"))
        );
        assert_eq!(
            response_pod.kvs().get(&Key::from("age")),
            Some(&PodValue::from(30))
        );
    }

    #[tokio::test]
    async fn test_hash_string_success() {
        let (server, _pool) = create_test_server_with_db().await;

        let input_string = "hello world";
        let response = server
            .post("/api/hash")
            .content_type("text/plain")
            .text(input_string.to_string())
            .await;

        assert_eq!(response.status_code(), StatusCode::OK);
        let response_hash: Hash = response.json();
        let expected_hash = hash_str(input_string);
        assert_eq!(response_hash, expected_hash);
    }

    #[tokio::test]
    async fn test_import_pod_to_space_and_delete() {
        let (server, db) = create_test_server_with_db().await;
        let space_id = "pod-space-for-import-delete"; // Unique name

        // Create space first
        insert_test_space(&db, space_id).await;

        let mut signer = MockSigner {
            pk: "0x1234567890abcdef".to_string(),
        };
        let params = middleware::Params::default();
        let mut pod_builder = SignedPodBuilder::new(&params);
        pod_builder.insert("test", "test");
        let pod = pod_builder.sign(&mut signer).unwrap();
        let pod_id = pod.id();
        let pod_id_string: String = pod_id.0.encode_hex::<String>();
        let import_payload = json!({
            "podType": "signed",
            "data": serde_json::to_value(pod).unwrap(),
            "label": "My Test Pod For Import Delete"
        });

        let response = server
            .post(&format!("/api/pods/{}", space_id))
            .json(&import_payload)
            .await;
        println!("Response: {:?}", response.text());
        assert_eq!(
            response.status_code(),
            StatusCode::CREATED,
            "Response: {:?}",
            response.text()
        );

        let pod_info_res = store::get_pod(&db, space_id, &pod_id_string).await;

        assert!(
            pod_info_res.is_ok(),
            "Pod not found in DB after import: {:?}",
            pod_info_res.err()
        );
        let pod_info = pod_info_res.unwrap().unwrap();
        assert_eq!(pod_info.id, pod_id_string);
        assert_eq!(pod_info.pod_type, "signed");

        // Assert data content
        // let expected_imported_helper: MainPodHelper = serde_json::from_value(import_main_pod_helper_data.clone()).unwrap();
        // let expected_pod_data_imported = PodData::Main(expected_imported_helper);
        // assert_eq!(pod_info.data, expected_pod_data_imported);

        let response = server
            .delete(&format!("/api/pods/{}/{}", space_id, pod_id_string))
            .await;
        assert_eq!(response.status_code(), StatusCode::NO_CONTENT);

        let response = server
            .get(&format!("/api/pods/{}/{}", space_id, pod_id_string))
            .await;
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }
}
