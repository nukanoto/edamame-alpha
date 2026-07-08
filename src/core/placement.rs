//! キャッシュノード状態に対する決定的なルーティングとプリウォーム計画。

use std::collections::BTreeSet;

use crate::core::registry::Registry;
use crate::domain::cache_node::{CacheNode, CacheNodeState};
use crate::domain::segment::SegmentId;

/// すべての ACTIVE ノードを `(load_score, node_id)` でソートし決定的に選択する
///（仕様 FR-019, FR-007）。
#[must_use]
pub fn active_nodes(registry: &Registry) -> Vec<&CacheNode> {
    let mut nodes: Vec<&CacheNode> = registry
        .nodes
        .values()
        .filter(|n| matches!(n.state, CacheNodeState::Active))
        .collect();
    nodes.sort_by(|a, b| {
        a.load_score
            .total_cmp(&b.load_score)
            .then_with(|| a.node_id.cmp(&b.node_id))
    });
    nodes
}

/// 生成されたメディアプレイリストをホストする単一の ACTIVE ノードを選択する（仕様 FR-031）。
///
/// ACTIVE ノードが利用できない場合は `None` を返す。呼び出し側はこれを `503` に
/// マッピングし、オリジンセグメントURLを露出させない（仕様 FR-022）。
#[must_use]
pub fn select_active(registry: &Registry) -> Option<&CacheNode> {
    active_nodes(registry).into_iter().next()
}

/// ノードが配信可能になる前にまだフェッチが必要なライブウィンドウセグメント。
///
/// DEAD および DRAINING ノードはプリウォーム対象にならない。COLD および ACTIVE ノードのみが
/// フルライブウィンドウにレプリケートされる（仕様 FR-011, FR-012）。
#[must_use]
pub fn prewarm_plan(node: &CacheNode, required: &BTreeSet<SegmentId>) -> Vec<SegmentId> {
    if matches!(node.state, CacheNodeState::Dead | CacheNodeState::Draining) {
        return Vec::new();
    }
    required.difference(&node.retained).cloned().collect()
}
