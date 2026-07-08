//! インジェスト、生成プレイリスト、レジストリ、ハートビートのAxum HTTPハンドラ。

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use serde::{Deserialize, Serialize};

use crate::core::{CoreState, ingest, manifest, placement};
use crate::domain::cache_node::{CacheNodeState, Health};
use crate::domain::rendition::RenditionMetadata;
use crate::domain::segment::{SegmentKey, render_filename};
use crate::error::EdamameError;
use crate::now_ms;

type Shared = Arc<CoreState>;

/// コアHTTPルーターを構築する。
pub fn router(state: Shared) -> Router {
    Router::new()
        .route(
            "/segments/{channel_id}/{rendition}/{filename}",
            put(put_segment).get(get_segment),
        )
        .route("/channels/{channel_id}/{file}", get(get_playlist))
        .route("/nodes/{node_id}", put(put_node))
        .route("/nodes/{node_id}/heartbeat", post(post_heartbeat))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct SegmentQuery {
    bandwidth: Option<u64>,
    resolution: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegisterNode {
    base_url: String,
    #[serde(default)]
    #[allow(dead_code)]
    capacity_hint: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct HeartbeatRequest {
    health: Health,
    load_score: f64,
    #[serde(default)]
    retained_segments: Vec<SegmentKey>,
}

#[derive(Debug, Serialize)]
struct HeartbeatResponse {
    state: CacheNodeState,
    prewarm: Vec<PrewarmItem>,
}

#[derive(Debug, Serialize)]
struct PrewarmItem {
    channel_id: String,
    rendition: String,
    filename: String,
}

fn m3u8_response(body: String) -> Response {
    (
        [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
        body,
    )
        .into_response()
}

async fn put_segment(
    State(state): State<Shared>,
    Path((channel, rendition, filename)): Path<(String, String, String)>,
    Query(query): Query<SegmentQuery>,
    body: Bytes,
) -> Result<Response, EdamameError> {
    let metadata = RenditionMetadata {
        bandwidth: query.bandwidth,
        resolution: query.resolution,
    };
    let now = now_ms();
    let mut inner = state.inner.lock().expect("core state mutex poisoned");
    let outcome = ingest::ingest_segment(
        &mut inner.channels,
        &state.store,
        &channel,
        &rendition,
        &filename,
        &metadata,
        &body,
        now,
    )?;
    let status = match outcome {
        ingest::IngestOutcome::Created => StatusCode::CREATED,
        ingest::IngestOutcome::AlreadyExists => StatusCode::OK,
    };
    Ok(status.into_response())
}

/// 正規セグメントを提供し、キャッシュノードがプリウォーム中にプルできるようにする（仕様 FR-013）。
async fn get_segment(
    State(state): State<Shared>,
    Path((channel, rendition, filename)): Path<(String, String, String)>,
) -> Result<Response, EdamameError> {
    let bytes = state.store.read_segment(&channel, &rendition, &filename)?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response())
}

async fn get_playlist(
    State(state): State<Shared>,
    Path((channel, file)): Path<(String, String)>,
) -> Result<Response, EdamameError> {
    let now = now_ms();
    let mut inner = state.inner.lock().expect("core state mutex poisoned");

    let required = ingest::required_segments(&inner.channels, state.window);
    inner.registry.evaluate_all(&state.eval, &required, now);

    let Some(node) = placement::select_active(&inner.registry).cloned() else {
        return Err(EdamameError::Unavailable(
            "no ACTIVE cache node is available".to_owned(),
        ));
    };

    let window = state.window;
    let crate::core::CoreInner {
        channels,
        snapshots,
        ..
    } = &mut *inner;
    let cstate = channels
        .get(&channel)
        .ok_or_else(|| EdamameError::NotFound(format!("unknown channel {channel}")))?;
    let snapshot = manifest::refresh_snapshot(snapshots, &channel, cstate, window)?;

    if file == "master.m3u8" {
        return Ok(m3u8_response(manifest::render_master_m3u8(
            &snapshot.master,
        )));
    }
    let rendition = file
        .strip_suffix(".m3u8")
        .ok_or_else(|| EdamameError::NotFound(format!("unknown playlist {file}")))?;
    let media = snapshot
        .media_for(rendition)
        .ok_or_else(|| EdamameError::NotFound(format!("unknown rendition {rendition}")))?;
    Ok(m3u8_response(manifest::render_media_m3u8(
        media,
        &node.base_url,
    )))
}

async fn put_node(
    State(state): State<Shared>,
    Path(node_id): Path<String>,
    Json(req): Json<RegisterNode>,
) -> Result<Response, EdamameError> {
    let now = now_ms();
    let mut inner = state.inner.lock().expect("core state mutex poisoned");
    let created = inner.registry.register(node_id, req.base_url, now);
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok(status.into_response())
}

async fn post_heartbeat(
    State(state): State<Shared>,
    Path(node_id): Path<String>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, EdamameError> {
    let now = now_ms();
    let mut retained = BTreeSet::new();
    for key in req.retained_segments {
        retained.insert(key.into_id()?);
    }

    let mut inner = state.inner.lock().expect("core state mutex poisoned");
    inner
        .registry
        .heartbeat(&node_id, req.health, req.load_score, retained, now)?;

    let required = ingest::required_segments(&inner.channels, state.window);
    inner.registry.evaluate_all(&state.eval, &required, now);

    let node = inner
        .registry
        .nodes
        .get(&node_id)
        .ok_or_else(|| EdamameError::NotFound(format!("unknown node {node_id}")))?;
    let state_now = node.state;
    let plan = placement::prewarm_plan(node, &required);

    let prewarm = plan
        .into_iter()
        .filter_map(|id| {
            let ext = inner
                .channels
                .get(&id.channel)
                .and_then(|c| c.renditions.get(&id.rendition))
                .and_then(|r| r.segments.get(&id.sequence))
                .map(|record| record.ext.clone())?;
            Some(PrewarmItem {
                channel_id: id.channel,
                rendition: id.rendition,
                filename: render_filename(id.sequence, &ext),
            })
        })
        .collect();

    Ok(Json(HeartbeatResponse {
        state: state_now,
        prewarm,
    }))
}
