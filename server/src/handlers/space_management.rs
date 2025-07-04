use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use super::AppError;
use crate::{
    api_types::SpaceInfo,
    db::{store, Db},
};

#[derive(Deserialize)]
pub struct CreateSpaceRequest {
    id: String,
}

// --- Space Management Handlers ---

// Handler for GET /api/spaces
pub async fn list_spaces(State(db): State<Arc<Db>>) -> Result<Json<Vec<SpaceInfo>>, AppError> {
    let spaces = store::list_spaces(&db).await?;
    Ok(Json(spaces))
}

// Handler for POST /api/spaces
pub async fn create_space(
    State(db): State<Arc<Db>>,
    Json(payload): Json<CreateSpaceRequest>,
) -> Result<impl IntoResponse, AppError> {
    store::create_space(&db, &payload.id).await?;
    Ok(StatusCode::CREATED)
}

// Handler for DELETE /api/spaces/:space_id
pub async fn delete_space(
    State(db): State<Arc<Db>>,
    Path(space_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let rows_deleted = store::delete_space(&db, &space_id).await?;

    if rows_deleted == 0 {
        Err(AppError::NotFound(format!(
            "Space with id '{}' not found",
            space_id
        )))
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

#[cfg(test)]
mod tests {
    // Renamed from space_management_tests to just tests for consistency
    use std::sync::Arc;

    use axum_test::TestServer;
    use hex::ToHex;
    use pod2::{
        backends::plonky2::mock::signedpod::MockSigner,
        frontend::SignedPodBuilder,
        middleware::{Params as PodParams, Value as PodValue},
    };
    use serde_json::{json, Value};

    use super::*;
    use crate::{
        api_types::{PodData, SpaceInfo},
        db,
        routes::create_router,
    };

    // Test helper to create a server
    pub async fn create_test_server() -> TestServer {
        let db = Arc::new(
            db::Db::new(None, &db::MIGRATIONS)
                .await
                .expect("Failed to init db for test"),
        );
        let router = create_router(db);
        TestServer::new(router).unwrap()
    }

    // Test helper to create a server and return the pool (useful for direct DB assertions)
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

    #[tokio::test]
    async fn test_create_list_delete_space() {
        let server = create_test_server().await;

        let response = server.get("/api/spaces").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        let spaces: Vec<SpaceInfo> = response.json();
        assert!(spaces.is_empty());

        let space_id = "my-test-space";
        let response = server
            .post("/api/spaces")
            .json(&json!({"id": space_id}))
            .await;
        assert_eq!(response.status_code(), StatusCode::CREATED);

        let response = server.get("/api/spaces").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        let spaces: Vec<SpaceInfo> = response.json();
        assert_eq!(spaces.len(), 1);
        assert_eq!(spaces[0].id, space_id);
        assert!(!spaces[0].created_at.is_empty());

        let response = server
            .post("/api/spaces")
            .json(&json!({"id": space_id}))
            .await;
        assert_ne!(response.status_code(), StatusCode::CREATED);
        assert_eq!(response.status_code(), StatusCode::INTERNAL_SERVER_ERROR);

        let response = server.delete(&format!("/api/spaces/{}", space_id)).await;
        assert_eq!(response.status_code(), StatusCode::NO_CONTENT);

        let response = server.get("/api/spaces").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        let spaces: Vec<SpaceInfo> = response.json();
        assert!(spaces.is_empty());

        let response = server.delete("/api/spaces/non-existent-space").await;
        assert_eq!(response.status_code(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_delete_space_cascades_pods() {
        let (server, db) = create_test_server_with_db().await;
        let space_id1 = "cascade-space-1";
        let space_id2 = "cascade-space-2";

        store::create_space(&db, space_id1)
            .await
            .expect("Failed to create space 1");
        store::create_space(&db, space_id2)
            .await
            .expect("Failed to create space 2");

        let test_params = PodParams::default();
        let (pod_id1, pod_data1) =
            create_sample_signed_pod_data(&test_params, "pod1", vec![("a", PodValue::from(1))]);
        let (pod_id2, pod_data2) =
            create_sample_signed_pod_data(&test_params, "pod2", vec![("b", PodValue::from(2))]);
        let (pod_id3, pod_data3) =
            create_sample_signed_pod_data(&test_params, "pod3", vec![("c", PodValue::from(3))]);

        let pod_data_enum1 = PodData::Signed(serde_json::from_value(pod_data1).unwrap());
        let pod_data_enum2 = PodData::Signed(serde_json::from_value(pod_data2).unwrap());
        let pod_data_enum3 = PodData::Signed(serde_json::from_value(pod_data3).unwrap());

        store::import_pod(&db, &pod_id1, "signed", &pod_data_enum1, None, space_id1)
            .await
            .unwrap();
        store::import_pod(&db, &pod_id2, "signed", &pod_data_enum2, None, space_id1)
            .await
            .unwrap();
        store::import_pod(&db, &pod_id3, "signed", &pod_data_enum3, None, space_id2)
            .await
            .unwrap();

        let pods_in_space1 = store::list_pods(&db, space_id1).await.unwrap();
        assert_eq!(pods_in_space1.len(), 2);

        let pods_in_space2 = store::list_pods(&db, space_id2).await.unwrap();
        assert_eq!(pods_in_space2.len(), 1);

        assert_eq!(
            server
                .delete(&format!("/api/spaces/{}", space_id1))
                .await
                .status_code(),
            StatusCode::NO_CONTENT
        );

        let pods_in_space1_after = store::list_pods(&db, space_id1).await.unwrap();
        assert!(pods_in_space1_after.is_empty());

        let pods_in_space2_after = store::list_pods(&db, space_id2).await.unwrap();
        assert_eq!(pods_in_space2_after.len(), 1);

        let space1_exists = store::space_exists(&db, space_id1).await.unwrap();
        assert!(!space1_exists);
    }
}
