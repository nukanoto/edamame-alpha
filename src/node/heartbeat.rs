//! ノード登録とハートビート/プリウォーム報告ループ。

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::segment::SegmentKey;
use crate::error::EdamameError;
use crate::node::{NodeState, prewarm};

#[derive(Debug, Serialize)]
struct RegisterBody {
    base_url: String,
    capacity_hint: u64,
}

#[derive(Debug, Serialize)]
struct HeartbeatBody {
    health: &'static str,
    load_score: f64,
    retained_segments: Vec<SegmentKey>,
}

/// ハートビート後にコントローラーがノードに返す状態。
#[derive(Debug, Deserialize)]
pub struct HeartbeatResponse {
    #[allow(dead_code)]
    pub state: String,
    pub prewarm: Vec<PrewarmItem>,
}

/// プリウォーム中にノードがコアからフェッチすべきセグメント。
#[derive(Debug, Deserialize)]
pub struct PrewarmItem {
    pub channel_id: String,
    pub rendition: String,
    pub filename: String,
}

fn core_base(state: &NodeState) -> &str {
    state.core_url.trim_end_matches('/')
}

/// このノードを COLD としてコアに登録する（HTTP契約: ノード登録）。
pub async fn register(state: &NodeState) -> Result<(), EdamameError> {
    let url = format!("{}/nodes/{}", core_base(state), state.node_id);
    let body = RegisterBody {
        base_url: state.base_url.clone(),
        capacity_hint: 100,
    };
    state
        .client
        .put(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| EdamameError::BadGateway(format!("node registration failed: {e}")))?;
    Ok(())
}

/// 1回のハートビートを送信し、コアが返したプリウォーム作業をすべてフェッチする。
pub async fn heartbeat_once(state: &NodeState, load_score: f64) -> Result<(), EdamameError> {
    let retained = {
        let cache = state.cache.lock().expect("node cache mutex poisoned");
        cache
            .retained_ids()
            .iter()
            .map(SegmentKey::from)
            .collect::<Vec<_>>()
    };

    let url = format!("{}/nodes/{}/heartbeat", core_base(state), state.node_id);
    let body = HeartbeatBody {
        health: "ok",
        load_score,
        retained_segments: retained,
    };
    let response = state
        .client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| EdamameError::BadGateway(format!("heartbeat failed: {e}")))?
        .error_for_status()
        .map_err(|e| EdamameError::BadGateway(format!("heartbeat rejected: {e}")))?
        .json::<HeartbeatResponse>()
        .await
        .map_err(|e| EdamameError::BadGateway(format!("heartbeat decode failed: {e}")))?;

    for item in response.prewarm {
        let origin_url = format!(
            "{}/segments/{}/{}/{}",
            core_base(state),
            item.channel_id,
            item.rendition,
            item.filename
        );
        prewarm::fetch_and_store(
            state,
            &item.channel_id,
            &item.rendition,
            &item.filename,
            &origin_url,
        )
        .await?;
    }
    Ok(())
}

/// 設定された間隔でハートビートループを永続的に実行する。
pub async fn run_loop(state: Arc<NodeState>, interval_ms: u64) {
    let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms.max(1)));
    loop {
        ticker.tick().await;
        if let Err(err) = heartbeat_once(&state, 0.0).await {
            eprintln!("heartbeat error: {err}");
        }
    }
}
