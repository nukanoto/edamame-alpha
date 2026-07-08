//! キャッシュノードデータプレーン。
//!
//! キャッシュノードは意図的に単純なデータプレーンとしている（仕様 FR-020）:
//! コアからセグメントをプリウォームし、保持セグメントを配信し、古いシーケンスを
//! エビクションし、ハートビート/メトリクスを報告する。**グローバルな**負荷分散、
//! クライアント割り当て、チャンネル状態管理、プレイリスト検証は行わない。
//! これらはコントローラーの責務のままである。

pub mod cache;
pub mod heartbeat;
pub mod prewarm;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::{get, put};

use cache::NodeCache;

/// 共有キャッシュノード状態。
pub struct NodeState {
    pub node_id: String,
    pub core_url: String,
    pub base_url: String,
    pub cache: Mutex<NodeCache>,
    pub client: reqwest::Client,
}

impl NodeState {
    /// デフォルトHTTPクライアントで共有ノード状態を構築する。
    #[must_use]
    pub fn new(node_id: String, core_url: String, base_url: String, cache: NodeCache) -> Arc<Self> {
        Arc::new(Self {
            node_id,
            core_url,
            base_url,
            cache: Mutex::new(cache),
            client: reqwest::Client::new(),
        })
    }
}

/// キャッシュノードHTTPルーターを構築する。
pub fn router(state: Arc<NodeState>) -> Router {
    Router::new()
        .route(
            "/prewarm/{channel_id}/{rendition}/{filename}",
            put(prewarm::put_prewarm),
        )
        .route(
            "/segments/{channel_id}/{rendition}/{filename}",
            get(cache::get_segment),
        )
        .with_state(state)
}
