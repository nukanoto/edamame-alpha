//! 決定的な ACTIVE ノード選択、ACTIVE ゼロの拒否、プリウォーム計画の単体テスト
//! （タスク T022, T035）。

use std::collections::BTreeSet;

use edamame_alpha::core::placement::{prewarm_plan, select_active};
use edamame_alpha::core::registry::Registry;
use edamame_alpha::domain::cache_node::{CacheNode, CacheNodeState, Health};
use edamame_alpha::domain::segment::SegmentId;

fn node(id: &str, state: CacheNodeState, load: f64, retained: &[u64]) -> CacheNode {
    CacheNode {
        node_id: id.to_owned(),
        base_url: format!("http://{id}"),
        state,
        last_heartbeat_ms: 0,
        health: Health::Ok,
        load_score: load,
        retained: retained
            .iter()
            .map(|s| SegmentId {
                channel: "live".to_owned(),
                rendition: "720p".to_owned(),
                sequence: *s,
            })
            .collect(),
    }
}

fn registry(nodes: Vec<CacheNode>) -> Registry {
    let mut reg = Registry::default();
    for n in nodes {
        reg.nodes.insert(n.node_id.clone(), n);
    }
    reg
}

fn required(seqs: &[u64]) -> BTreeSet<SegmentId> {
    seqs.iter()
        .map(|s| SegmentId {
            channel: "live".to_owned(),
            rendition: "720p".to_owned(),
            sequence: *s,
        })
        .collect()
}

#[test]
fn selects_lowest_load_active_node_deterministically() {
    let reg = registry(vec![
        node("cache-b", CacheNodeState::Active, 0.50, &[]),
        node("cache-a", CacheNodeState::Active, 0.20, &[]),
        node("cache-c", CacheNodeState::Draining, 0.10, &[]),
    ]);
    // 最も低い負荷が勝つ; cache-c は DRAINING で対象外。
    let chosen = select_active(&reg).unwrap();
    assert_eq!(chosen.node_id, "cache-a");

    // 選択は繰り返し呼び出しでも安定している。
    assert_eq!(select_active(&reg).unwrap().node_id, "cache-a");
}

#[test]
fn no_active_node_yields_none() {
    let reg = registry(vec![
        node("cache-a", CacheNodeState::Cold, 0.0, &[]),
        node("cache-b", CacheNodeState::Draining, 0.0, &[]),
        node("cache-c", CacheNodeState::Dead, 0.0, &[]),
    ]);
    assert!(select_active(&reg).is_none());
}

#[test]
fn prewarm_plan_lists_missing_segments_for_eligible_nodes() {
    let cold = node("cache-a", CacheNodeState::Cold, 0.0, &[101]);
    let plan = prewarm_plan(&cold, &required(&[101, 102, 103]));
    let seqs: Vec<u64> = plan.iter().map(|id| id.sequence).collect();
    assert_eq!(seqs, vec![102, 103]);
}

#[test]
fn prewarm_plan_skips_draining_and_dead_nodes() {
    let draining = node("cache-a", CacheNodeState::Draining, 0.0, &[]);
    assert!(prewarm_plan(&draining, &required(&[101])).is_empty());
    let dead = node("cache-b", CacheNodeState::Dead, 0.0, &[]);
    assert!(prewarm_plan(&dead, &required(&[101])).is_empty());
}
