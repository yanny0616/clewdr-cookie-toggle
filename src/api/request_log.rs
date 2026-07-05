use axum::{Json, extract::Query};
use serde::Deserialize;

use crate::services::request_log::{self, ApiRequestLog};

#[derive(Debug, Deserialize)]
pub struct RequestLogQuery {
    pub limit: Option<usize>,
}

pub async fn api_get_request_logs(
    Query(query): Query<RequestLogQuery>,
) -> Json<Vec<ApiRequestLog>> {
    Json(request_log::list(query.limit).await)
}

pub async fn api_clear_request_logs() -> Json<serde_json::Value> {
    request_log::clear().await;
    Json(serde_json::json!({ "ok": true }))
}
