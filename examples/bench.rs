//! CPU拘束のコアパス向けの軽量で依存関係のないマイクロベンチマーク。
//!
//! 実行方法: `cargo run --release --example bench`
//!
//! これらはアドホックなタイミングループ（criterion なし）であり、純粋な計画/
//! マニフェスト関数のコストをおおよそ感じるためだけのものです。常に `--release`
//! の数値を読んでください; デバッグのタイミングは参考になりません。

use std::collections::{BTreeMap, BTreeSet};
use std::hint::black_box;
use std::time::Instant;

use edamame_alpha::core::ingest::{ChannelState, RenditionState, SegmentRecord};
use edamame_alpha::core::manifest::{regenerate, render_master_m3u8, render_media_m3u8};
use edamame_alpha::core::placement::{active_nodes, prewarm_plan};
use edamame_alpha::core::registry::Registry;
use edamame_alpha::domain::cache_node::{CacheNode, CacheNodeState, Health};
use edamame_alpha::domain::rendition::RenditionMetadata;
use edamame_alpha::domain::segment::SegmentId;
use edamame_alpha::domain::window::WindowConfig;

/// rng 依存関係なしで実行を再現可能にする小さな決定的LCG。
struct Lcg(u64);
impl Lcg {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0
    }
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

const WINDOW: WindowConfig = WindowConfig {
    window_size: 6,
    retention_window: 12,
};

/// 短いウォームアップ後、固定の壁時計予算で `f` を実行しスループットを報告する。
fn bench(name: &str, mut f: impl FnMut()) {
    // ウォームアップ。
    for _ in 0..1_000 {
        f();
    }
    // 計測: ~200ms経過するまで続ける。
    let budget = std::time::Duration::from_millis(200);
    let mut iters: u64 = 0;
    let start = Instant::now();
    while start.elapsed() < budget {
        for _ in 0..1_000 {
            f();
        }
        iters += 1_000;
    }
    let elapsed = start.elapsed();
    let per = elapsed.as_secs_f64() / iters as f64;
    let ops = iters as f64 / elapsed.as_secs_f64();
    println!(
        "{name:<34} {iters:>10} iters  {:>9.0} ns/op  {:>12.0} ops/s",
        per * 1e9,
        ops,
    );
}

/// `renditions` 個のレンディションを持つチャンネルを構築し、各レンディションが
/// 共有ライブエッジまで `committed` 個の連続したコミット済みセグメントを保持する。
fn make_channel(renditions: usize, committed: u64) -> ChannelState {
    let mut cstate = ChannelState::default();
    for r in 0..renditions {
        let mut rstate = RenditionState {
            metadata: RenditionMetadata {
                bandwidth: Some(1_000_000 + r as u64 * 250_000),
                resolution: Some(format!("{}x{}", 640 + r * 320, 360 + r * 180)),
            },
            ..Default::default()
        };
        for seq in 0..committed {
            rstate.segments.insert(
                seq,
                SegmentRecord {
                    ext: "ts".to_owned(),
                    hash: seq.wrapping_mul(2654435761),
                },
            );
        }
        cstate.renditions.insert(format!("r{r}"), rstate);
    }
    cstate
}

fn make_registry(node_count: usize, retained_per_node: usize) -> Registry {
    let mut rng = Lcg(0x1234_5678);
    let mut registry = Registry::default();
    for n in 0..node_count {
        let mut retained = BTreeSet::new();
        for s in 0..retained_per_node {
            retained.insert(SegmentId {
                channel: "live".to_owned(),
                rendition: format!("r{}", s % 4),
                sequence: s as u64,
            });
        }
        let node = CacheNode {
            node_id: format!("node-{n:04}"),
            base_url: format!("http://10.0.0.{}:8080", n % 250),
            // 状態の混在により active_nodes もフィルタリングする必要がある。
            state: if n % 5 == 0 {
                CacheNodeState::Cold
            } else {
                CacheNodeState::Active
            },
            last_heartbeat_ms: 0,
            health: Health::Ok,
            load_score: rng.next_f64() * 100.0,
            retained,
        };
        registry.nodes.insert(node.node_id.clone(), node);
    }
    registry
}

fn main() {
    let release = !cfg!(debug_assertions);
    println!(
        "edamame core micro-benchmarks  (build: {})\n",
        if release { "release" } else { "DEBUG — numbers not meaningful" }
    );

    // --- マニフェスト再生成 -------------------------------------------------
    for &(rends, segs) in &[(1u64, 6u64), (4, 6), (8, 12)] {
        let cstate = make_channel(rends as usize, segs);
        bench(&format!("regenerate({rends} rend, {segs} seg)"), || {
            black_box(regenerate("live", black_box(&cstate), WINDOW).unwrap());
        });
    }

    // --- m3u8 レンダリング --------------------------------------------------------
    {
        let cstate = make_channel(4, 6);
        let snap = regenerate("live", &cstate, WINDOW).unwrap();
        let media = &snap.media[0];
        bench("render_media_m3u8(6 seg)", || {
            black_box(render_media_m3u8(black_box(media), "http://node-1:8080"));
        });
        bench("render_master_m3u8(4 rend)", || {
            black_box(render_master_m3u8(black_box(&snap.master)));
        });
    }

    // --- 多数チャンネルでの required_segments ----------------------------------
    for &chans in &[1usize, 16, 64] {
        let mut channels: BTreeMap<String, ChannelState> = BTreeMap::new();
        for c in 0..chans {
            channels.insert(format!("ch{c:03}"), make_channel(4, 12));
        }
        bench(&format!("required_segments({chans} chan x4 rend)"), || {
            black_box(edamame_alpha::core::ingest::required_segments(
                black_box(&channels),
                WINDOW,
            ));
        });
    }

    // --- ノードレジストリ上の配置 --------------------------------------
    for &nodes in &[8usize, 64, 512] {
        let registry = make_registry(nodes, 24);
        bench(&format!("active_nodes({nodes} nodes)"), || {
            black_box(active_nodes(black_box(&registry)));
        });
    }

    // --- プリウォーム計画 ------------------------------------------------------
    {
        let registry = make_registry(1, 12);
        let node = registry.nodes.values().next().unwrap();
        let mut required = BTreeSet::new();
        for s in 0..48usize {
            required.insert(SegmentId {
                channel: "live".to_owned(),
                rendition: format!("r{}", s % 4),
                sequence: s as u64,
            });
        }
        bench("prewarm_plan(48 req / 12 held)", || {
            black_box(prewarm_plan(black_box(node), black_box(&required)));
        });
    }
}
