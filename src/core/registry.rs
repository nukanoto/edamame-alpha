//! キャッシュノードレジストリと決定的なオンデマンド状態評価（仕様 FR-042）。

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::cache_node::{CacheNode, CacheNodeState, Health};
use crate::domain::segment::SegmentId;
use crate::error::EdamameError;

/// 状態評価の調整可能な閾値。
#[derive(Debug, Clone, Copy)]
pub struct EvalParams {
    pub heartbeat_timeout_ms: u64,
    pub overload_threshold: f64,
}

/// コントローラーのキャッシュノードレジストリ。
#[derive(Debug, Default)]
pub struct Registry {
    pub nodes: BTreeMap<String, CacheNode>,
}

impl Registry {
    /// ノードを登録または既存ノードを更新する。新規ノードの場合は `true` を返す。
    pub fn register(&mut self, node_id: String, base_url: String, now_ms: u64) -> bool {
        match self.nodes.get_mut(&node_id) {
            Some(node) => {
                node.base_url = base_url;
                node.last_heartbeat_ms = now_ms;
                false
            }
            None => {
                self.nodes.insert(
                    node_id.clone(),
                    CacheNode::new_cold(node_id, base_url, now_ms),
                );
                true
            }
        }
    }

    /// ハートビート更新を適用する。ハートビートを再開した DEAD ノードは COLD に戻り、
    /// ACTIVE になる前に再度プリウォームを完了させる必要がある（仕様 FR-032）。
    pub fn heartbeat(
        &mut self,
        node_id: &str,
        health: Health,
        load_score: f64,
        retained: BTreeSet<SegmentId>,
        now_ms: u64,
    ) -> Result<(), EdamameError> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| EdamameError::NotFound(format!("unknown node {node_id}")))?;
        node.last_heartbeat_ms = now_ms;
        node.health = health;
        node.load_score = load_score;
        if matches!(node.state, CacheNodeState::Dead) {
            // ハートビートを再開した DEAD ノードは COLD に戻り、ACTIVE になる前に
            // 再度プリウォームを完了させる必要がある。再開時に即座に ACTIVE 化されないよう、
            // 以前報告された保持セットを破棄する（仕様 FR-032）。
            node.state = CacheNodeState::Cold;
            node.retained.clear();
        } else {
            node.retained = retained;
        }
        Ok(())
    }

    /// すべてのノードを現在の必須セグメントと時刻に対して再評価する。
    pub fn evaluate_all(&mut self, eval: &EvalParams, required: &BTreeSet<SegmentId>, now_ms: u64) {
        for node in self.nodes.values_mut() {
            evaluate_state(node, eval, required, now_ms);
        }
    }
}

/// 単一ノードの状態を1回の評価パスで決定的に遷移させる。
///
/// この関数は時計を読み取らない。`now_ms` が外部から供給されるため、
/// 同一の `(node, required, now)` 入力に対して常に同じ結果が得られる（仕様 FR-042）。
pub fn evaluate_state(
    node: &mut CacheNode,
    eval: &EvalParams,
    required: &BTreeSet<SegmentId>,
    now_ms: u64,
) {
    if now_ms.saturating_sub(node.last_heartbeat_ms) > eval.heartbeat_timeout_ms {
        node.state = CacheNodeState::Dead;
        return;
    }

    let overloaded = node.load_score >= eval.overload_threshold;
    let healthy = matches!(node.health, Health::Ok);
    let has_all = node.has_all(required);

    node.state = match node.state {
        CacheNodeState::Cold => {
            if healthy && !overloaded && has_all {
                CacheNodeState::Active
            } else {
                CacheNodeState::Cold
            }
        }
        CacheNodeState::Active => {
            if overloaded {
                CacheNodeState::Draining
            } else if !has_all || !healthy {
                CacheNodeState::Cold
            } else {
                CacheNodeState::Active
            }
        }
        CacheNodeState::Draining => {
            if !overloaded && healthy && has_all {
                CacheNodeState::Active
            } else {
                CacheNodeState::Draining
            }
        }
        CacheNodeState::Dead => CacheNodeState::Dead,
    };
}
