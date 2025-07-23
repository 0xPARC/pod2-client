use axum::response::Json;
use podnet_models::ServerInfo;

pub async fn root() -> Json<ServerInfo> {
    let public_key = crate::pod::get_server_public_key();
    Json(ServerInfo { public_key })
}
