//! ライブウィンドウ範囲と保持計算（仕様 FR-014, FR-015）。

use crate::error::EdamameError;

/// コアで共有され、チャンネルごとに選択される静的なウィンドウ設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowConfig {
    /// ライブエッジで保持するセグメント数。
    pub window_size: u64,
    /// エビクション前にライブエッジから許容される距離。
    pub retention_window: u64,
}

impl WindowConfig {
    /// ウィンドウと保持サイズの関係を検証する。
    pub fn validate(&self) -> Result<(), EdamameError> {
        if self.window_size == 0 {
            return Err(EdamameError::BadRequest(
                "live window size must be greater than zero".to_owned(),
            ));
        }
        if self.retention_window < self.window_size {
            return Err(EdamameError::BadRequest(
                "retention window must be at least the live window size".to_owned(),
            ));
        }
        Ok(())
    }
}

/// 現在のライブエッジを基点とした具体的なライブウィンドウ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveWindow {
    pub live_edge: u64,
    pub config: WindowConfig,
}

impl LiveWindow {
    /// ライブウィンドウに属する最初のシーケンス。
    #[must_use]
    pub fn live_start(&self) -> u64 {
        self.live_edge
            .saturating_sub(self.config.window_size.saturating_sub(1))
    }

    /// ライブウィンドウシーケンスの包含的な `[live_start, live_edge]` 範囲。
    #[must_use]
    pub fn live_range(&self) -> std::ops::RangeInclusive<u64> {
        self.live_start()..=self.live_edge
    }

    /// シーケンスが `live_edge - retention_window` より古い場合に true。
    #[must_use]
    pub fn is_evictable(&self, sequence: u64) -> bool {
        sequence < self.live_edge.saturating_sub(self.config.retention_window)
    }
}
