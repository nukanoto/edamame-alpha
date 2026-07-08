//! ノードプリウォーム: コアから正規セグメントをプルして保持する。

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::domain::segment::{SegmentId, parse_filename};
use crate::error::EdamameError;
use crate::node::NodeState;

/// プリウォーム指示のボディ（仕様 FR-029）: セグメントをプルする元URL。
#[derive(Debug, Deserialize)]
pub struct PrewarmRequest {
    pub origin_url: String,
}

/// `PUT /prewarm/{channel}/{rendition}/{filename}` — セグメントをフェッチして保持する。
pub async fn put_prewarm(
    State(state): State<Arc<NodeState>>,
    Path((channel, rendition, filename)): Path<(String, String, String)>,
    Json(req): Json<PrewarmRequest>,
) -> Result<Response, EdamameError> {
    fetch_and_store(&state, &channel, &rendition, &filename, &req.origin_url).await?;
    Ok(StatusCode::ACCEPTED.into_response())
}

/// `origin_url` からセグメントをフェッチし、ノードキャッシュに保存する。
///
/// フェッチ失敗はすべて `502 Bad Gateway` にマッピングする（HTTP契約: プリウォーム）。
pub async fn fetch_and_store(
    state: &NodeState,
    channel: &str,
    rendition: &str,
    filename: &str,
    origin_url: &str,
) -> Result<(), EdamameError> {
    let (sequence, ext) = parse_filename(filename)?;
    let id = SegmentId::new(channel, rendition, sequence)?;

    let response = state
        .client
        .get(origin_url)
        .send()
        .await
        .map_err(|e| EdamameError::BadGateway(format!("prewarm fetch failed: {e}")))?;
    if !response.status().is_success() {
        return Err(EdamameError::BadGateway(format!(
            "prewarm fetch returned status {}",
            response.status()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| EdamameError::BadGateway(format!("prewarm body read failed: {e}")))?;

    let mut cache = state.cache.lock().expect("node cache mutex poisoned");
    cache.store(id, &ext, &bytes)?;
    Ok(())
}
