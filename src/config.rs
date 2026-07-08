//! `edamame-core` と `edamame-node` バイナリのCLI設定。

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

use crate::domain::window::WindowConfig;
use crate::error::EdamameError;

/// 統合オリジン/コントローラープロセスの設定。
#[derive(Debug, Clone, Parser)]
#[command(
    name = "edamame-core",
    about = "Edamame MVP combined origin/controller"
)]
pub struct CoreConfig {
    /// HTTPサーバーのバインド先アドレス。例: `127.0.0.1:9000`。
    #[arg(long)]
    pub bind: SocketAddr,

    /// 正規セグメントのストレージディレクトリ。
    #[arg(long)]
    pub storage_dir: PathBuf,

    /// ライブエッジで保持するセグメント数。
    #[arg(long)]
    pub live_window: u64,

    /// エビクション前にライブエッジから許容される距離。
    #[arg(long)]
    pub retention_window: u64,

    /// ノードを DEAD と見なすハートビートタイムアウト（ミリ秒）。
    #[arg(long)]
    pub heartbeat_timeout_ms: u64,

    /// ノードが過負荷と見なされる負荷スコアの閾値以上。
    #[arg(long)]
    pub overload_threshold: f64,
}

impl CoreConfig {
    /// 検証済みのウィンドウ設定。
    pub fn window(&self) -> Result<WindowConfig, EdamameError> {
        let window = WindowConfig {
            window_size: self.live_window,
            retention_window: self.retention_window,
        };
        window.validate()?;
        Ok(window)
    }

    /// 設定を検証し、不整合な値の場合は起動を拒否する
    ///（CLI契約: 保持ウィンドウはライブウィンドウより小さくしてはいけない）。
    pub fn validate(&self) -> Result<(), EdamameError> {
        self.window()?;
        if !(0.0..=1.0).contains(&self.overload_threshold) {
            return Err(EdamameError::BadRequest(
                "overload threshold must be between 0.0 and 1.0".to_owned(),
            ));
        }
        Ok(())
    }
}

/// キャッシュノードエージェントの設定。
#[derive(Debug, Clone, Parser)]
#[command(name = "edamame-node", about = "Edamame MVP cache node")]
pub struct NodeConfig {
    /// コアに対して公開される安定した識別子。
    #[arg(long)]
    pub node_id: String,

    /// ノードHTTPサーバーのバインド先アドレス。
    #[arg(long)]
    pub bind: SocketAddr,

    /// コアのベースURL。例: `http://127.0.0.1:9000`。
    #[arg(long)]
    pub core_url: String,

    /// 保持セグメントのストレージディレクトリ。
    #[arg(long)]
    pub cache_dir: PathBuf,

    /// ハートビート報告間隔（ミリ秒）。
    #[arg(long)]
    pub heartbeat_interval_ms: u64,
}
