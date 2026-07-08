//! キャッシュノードの識別子、ライフサイクル状態、健全性、保持セグメントセット。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::domain::segment::SegmentId;

/// ノードが新しいクライアント割り当てを受けるかどうかを制御するライフサイクル状態
///（仕様 FR-006）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CacheNodeState {
    /// プリウォーム中; クライアントプレイリスト割り当て対象外。
    Cold,
    /// 新しいクライアントプレイリスト割り当て対象。
    Active,
    /// 以前のトラフィックが終了するまで将来のプレイリストから除外される。
    Draining,
    /// ハートビート欠落; プリウォームやプレイリスト書き換えには使用されない。
    Dead,
}

/// キャッシュノードが報告する健全性状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Ok,
    Bad,
}

/// 登録済みキャッシュノードとその最後に報告された状態。
#[derive(Debug, Clone)]
pub struct CacheNode {
    pub node_id: String,
    pub base_url: String,
    pub state: CacheNodeState,
    pub last_heartbeat_ms: u64,
    pub health: Health,
    pub load_score: f64,
    pub retained: BTreeSet<SegmentId>,
}

impl CacheNode {
    /// COLD 状態で新規登録されたノードを作成する（仕様 FR-010）。
    #[must_use]
    pub fn new_cold(node_id: String, base_url: String, now_ms: u64) -> Self {
        Self {
            node_id,
            base_url,
            state: CacheNodeState::Cold,
            last_heartbeat_ms: now_ms,
            health: Health::Ok,
            load_score: 0.0,
            retained: BTreeSet::new(),
        }
    }

    /// ノードが現在必要なライブウィンドウセグメントをすべて保持している場合に true。
    #[must_use]
    pub fn has_all(&self, required: &BTreeSet<SegmentId>) -> bool {
        required.is_subset(&self.retained)
    }
}
