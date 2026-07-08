//! キャッシュノード状態遷移、過負荷処理、注入された時刻によるハートビート欠落タイムアウト、
//! キャッシュエビクションの単体テスト（タスク T034, T046）。

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use edamame_alpha::core::registry::{EvalParams, evaluate_state};
use edamame_alpha::domain::cache_node::{CacheNode, CacheNodeState, Health};
use edamame_alpha::domain::segment::SegmentId;
use edamame_alpha::node::cache::NodeCache;

fn params() -> EvalParams {
    EvalParams {
        heartbeat_timeout_ms: 10_000,
        overload_threshold: 0.80,
    }
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

fn node(state: CacheNodeState, retained: &[u64]) -> CacheNode {
    CacheNode {
        node_id: "cache-a".to_owned(),
        base_url: "http://127.0.0.1:9001".to_owned(),
        state,
        last_heartbeat_ms: 1_000,
        health: Health::Ok,
        load_score: 0.0,
        retained: required(retained),
    }
}

#[test]
fn cold_becomes_active_when_prewarm_complete() {
    let mut n = node(CacheNodeState::Cold, &[101, 102]);
    evaluate_state(&mut n, &params(), &required(&[101, 102]), 1_000);
    assert_eq!(n.state, CacheNodeState::Active);
}

#[test]
fn cold_stays_cold_when_missing_required_segment() {
    let mut n = node(CacheNodeState::Cold, &[101]);
    evaluate_state(&mut n, &params(), &required(&[101, 102]), 1_000);
    assert_eq!(n.state, CacheNodeState::Cold);
}

#[test]
fn active_overloaded_moves_to_draining() {
    let mut n = node(CacheNodeState::Active, &[101, 102]);
    n.load_score = 0.95;
    evaluate_state(&mut n, &params(), &required(&[101, 102]), 1_000);
    assert_eq!(n.state, CacheNodeState::Draining);
}

#[test]
fn active_missing_segment_returns_to_cold() {
    let mut n = node(CacheNodeState::Active, &[101]);
    evaluate_state(&mut n, &params(), &required(&[101, 102]), 1_000);
    assert_eq!(n.state, CacheNodeState::Cold);
}

#[test]
fn draining_recovers_to_active_when_load_and_health_ok() {
    let mut n = node(CacheNodeState::Draining, &[101, 102]);
    n.load_score = 0.10;
    evaluate_state(&mut n, &params(), &required(&[101, 102]), 1_000);
    assert_eq!(n.state, CacheNodeState::Active);
}

#[test]
fn missing_heartbeat_marks_dead_using_injected_time() {
    let mut n = node(CacheNodeState::Active, &[101, 102]);
    // last_heartbeat_ms は 1_000; タイムアウトウィンドウを超えるまで now を進める。
    evaluate_state(&mut n, &params(), &required(&[101, 102]), 1_000 + 10_001);
    assert_eq!(n.state, CacheNodeState::Dead);
}

#[test]
fn fresh_heartbeat_does_not_trip_timeout() {
    let mut n = node(CacheNodeState::Active, &[101, 102]);
    evaluate_state(&mut n, &params(), &required(&[101, 102]), 1_000 + 10_000);
    assert_eq!(n.state, CacheNodeState::Active);
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_cache_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "edamame-evict-{}-{}-{}",
        std::process::id(),
        nanos,
        n
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn evicts_sequences_older_than_live_edge_minus_retention() {
    let dir = temp_cache_dir();
    let mut cache = NodeCache::new(dir.clone(), 12);
    for seq in 1..=20u64 {
        let id = SegmentId {
            channel: "live".to_owned(),
            rendition: "720p".to_owned(),
            sequence: seq,
        };
        cache.store(id, "ts", b"bytes").unwrap();
    }
    let retained: BTreeSet<u64> = cache
        .retained_ids()
        .into_iter()
        .map(|id| id.sequence)
        .collect();
    // live_edge = 20, retention = 12 => シーケンス < 8 をエビクションする。
    assert!(!retained.contains(&7));
    assert!(retained.contains(&8));
    assert!(retained.contains(&20));
    let _ = std::fs::remove_dir_all(dir);
}
