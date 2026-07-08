//! 共有テストフィクスチャ: 一時ディレクトリ、サンプルバイト列、エフェメラルなローカルサーバー。

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use edamame_alpha::core::registry::EvalParams;
use edamame_alpha::core::storage::CanonicalStore;
use edamame_alpha::core::{CoreState, router as core_router};
use edamame_alpha::domain::window::WindowConfig;
use edamame_alpha::node::cache::NodeCache;
use edamame_alpha::node::{NodeState, router as node_router};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// ドロップ時に削除される一時ディレクトリ。
pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    pub fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "edamame-{}-{}-{}-{}",
            tag,
            std::process::id(),
            nanos,
            n
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// シードに対する決定的なサンプルセグメントバイト列。
pub fn sample_bytes(seed: &str) -> Vec<u8> {
    format!("edamame-segment-{seed}").into_bytes()
}

/// エフェメラルなローカルポートにバインドされた実行中のコアサーバー。
pub struct CoreHandle {
    pub base_url: String,
    pub state: Arc<CoreState>,
    _tmp: TempDir,
}

/// 指定されたウィンドウと評価パラメータでコアサーバーを起動する。
pub async fn spawn_core(window: WindowConfig, eval: EvalParams) -> CoreHandle {
    let tmp = TempDir::new("core");
    let store = CanonicalStore::new(tmp.path.clone());
    let state = CoreState::new(store, window, eval);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = core_router(state.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    CoreHandle {
        base_url: format!("http://{addr}"),
        state,
        _tmp: tmp,
    }
}

/// ハートビートループなしで実行中のキャッシュノードサーバー（制御されたテスト用）。
pub struct NodeHandle {
    pub base_url: String,
    pub state: Arc<NodeState>,
    _tmp: TempDir,
}

/// エフェメラルなローカルポートにバインドされたキャッシュノードサーバーを起動する。
pub async fn spawn_node(node_id: &str, core_url: &str, retention_window: u64) -> NodeHandle {
    let tmp = TempDir::new("node");
    let cache = NodeCache::new(tmp.path.clone(), retention_window);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let state = NodeState::new(
        node_id.to_owned(),
        core_url.to_owned(),
        base_url.clone(),
        cache,
    );
    let app = node_router(state.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    NodeHandle {
        base_url,
        state,
        _tmp: tmp,
    }
}

/// 便利関数: デフォルトのMVPウィンドウ（ライブ 4、保持 8）。
pub fn default_window() -> WindowConfig {
    WindowConfig {
        window_size: 4,
        retention_window: 8,
    }
}

/// 便利関数: デフォルトのMVP評価パラメータ。
pub fn default_eval() -> EvalParams {
    EvalParams {
        heartbeat_timeout_ms: 10_000,
        overload_threshold: 0.80,
    }
}
