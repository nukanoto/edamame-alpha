//! 統合オリジン + コントローラープロセスの状態とHTTPサーフェス。
//!
//! オリジンの責務（正規ストレージ、生成プレイリストの検証）と
//! コントローラーの責務（キャッシュノードレジストリ、状態評価、プリウォーム
//! 計画、マニフェスト書き換え）は、1つの `edamame-core` プロセスが両方を
//! ホストしていても、個別に識別・テストできるよう別モジュールにしています。

pub mod http;
pub mod ingest;
pub mod manifest;
pub mod placement;
pub mod registry;
pub mod storage;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::domain::playlist::ChannelSnapshot;
use crate::domain::window::WindowConfig;
use ingest::ChannelState;
use registry::{EvalParams, Registry};
use storage::CanonicalStore;

pub use http::router;

/// コアプロセスの共有・ロック可能な状態。
pub struct CoreState {
    pub store: CanonicalStore,
    pub window: WindowConfig,
    pub eval: EvalParams,
    pub inner: Mutex<CoreInner>,
}

/// [`CoreState`] のミューテックスで保護された可変状態。
#[derive(Default)]
pub struct CoreInner {
    pub channels: BTreeMap<String, ChannelState>,
    pub registry: Registry,
    pub snapshots: BTreeMap<String, ChannelSnapshot>,
}

impl CoreState {
    /// 正規ストレージと調整パラメータから共有コア状態を構築する。
    #[must_use]
    pub fn new(store: CanonicalStore, window: WindowConfig, eval: EvalParams) -> Arc<Self> {
        Arc::new(Self {
            store,
            window,
            eval,
            inner: Mutex::new(CoreInner::default()),
        })
    }
}
