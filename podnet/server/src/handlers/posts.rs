use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use podnet_models::PostWithDocuments;

pub async fn get_posts(
    State(state): State<Arc<crate::AppState>>,
) -> Result<Json<Vec<PostWithDocuments>>, StatusCode> {
    let posts = state
        .db
        .get_all_posts()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut posts_with_documents = Vec::new();
    for post in posts {
        if post.id.is_none() {
            continue; // Skip posts without an ID
        }

        let post_id = post.id.unwrap();
        let documents_metadata = state
            .db
            .get_documents_metadata_by_post_id(post_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        posts_with_documents.push(PostWithDocuments {
            id: post.id,
            created_at: post.created_at,
            last_edited_at: post.last_edited_at,
            documents: documents_metadata,
        });
    }
    Ok(Json(posts_with_documents))
}

async fn get_post_with_documents_from_db(
    post_id: i64,
    state: Arc<crate::AppState>,
) -> Result<PostWithDocuments, StatusCode> {
    let post = state
        .db
        .get_post(post_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let documents_metadata = state
        .db
        .get_documents_metadata_by_post_id(post_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(PostWithDocuments {
        id: post.id,
        created_at: post.created_at,
        last_edited_at: post.last_edited_at,
        documents: documents_metadata,
    })
}

pub async fn get_post_by_id(
    Path(id): Path<i64>,
    State(state): State<Arc<crate::AppState>>,
) -> Result<Json<PostWithDocuments>, StatusCode> {
    let post_with_documents = get_post_with_documents_from_db(id, state).await?;
    Ok(Json(post_with_documents))
}
