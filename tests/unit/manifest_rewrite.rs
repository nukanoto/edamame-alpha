//! 単一の ACTIVE ノードへのメディアプレイリスト書き換えと、失敗時に前のスナップショットを
//! 保持するアトミックな最新プレイリスト入れ替えの単体テスト
//! （タスク T021, T022a）。

use std::collections::BTreeMap;

use edamame_alpha::core::ingest::{ChannelState, RenditionState, SegmentRecord};
use edamame_alpha::core::manifest::{generate_media_playlist, refresh_snapshot, render_media_m3u8};
use edamame_alpha::domain::playlist::ChannelSnapshot;
use edamame_alpha::domain::rendition::RenditionMetadata;
use edamame_alpha::domain::window::WindowConfig;

fn rendition_state(seqs: &[u64]) -> RenditionState {
    let mut segments = BTreeMap::new();
    for s in seqs {
        segments.insert(
            *s,
            SegmentRecord {
                ext: "ts".to_owned(),
                hash: *s,
            },
        );
    }
    RenditionState {
        metadata: RenditionMetadata::default(),
        segments,
        commits: Vec::new(),
    }
}

fn window() -> WindowConfig {
    WindowConfig {
        window_size: 2,
        retention_window: 4,
    }
}

#[test]
fn media_playlist_rewrites_to_single_active_node() {
    let rstate = rendition_state(&[101, 102]);
    let playlist = generate_media_playlist("live", "720p", &rstate, window()).unwrap();
    let m3u8 = render_media_m3u8(&playlist, "http://127.0.0.1:9001/");
    assert!(m3u8.contains("#EXT-X-MEDIA-SEQUENCE:101"));
    assert!(m3u8.contains("http://127.0.0.1:9001/segments/live/720p/101.ts"));
    assert!(m3u8.contains("http://127.0.0.1:9001/segments/live/720p/102.ts"));
    // すべてのセグメントURLが同一ノードを指す。
    assert_eq!(m3u8.matches("http://127.0.0.1:9001/").count(), 2);
}

fn channel_with(renditions: &[(&str, &[u64])]) -> ChannelState {
    let mut cstate = ChannelState::default();
    for (name, seqs) in renditions {
        cstate
            .renditions
            .insert((*name).to_owned(), rendition_state(seqs));
    }
    cstate
}

#[test]
fn refresh_replaces_only_after_validation_succeeds() {
    let mut snapshots: BTreeMap<String, ChannelSnapshot> = BTreeMap::new();

    // 最初の有効な生成: 両方のレンディションが同一範囲 (101..=102) を示す。
    let valid = channel_with(&[("720p", &[101, 102]), ("1080p", &[101, 102])]);
    let first = refresh_snapshot(&mut snapshots, "live", &valid, window())
        .unwrap()
        .clone();
    assert_eq!(first.media.len(), 2);
    assert!(snapshots.contains_key("live"));

    // 不一致な状態: レンディションが異なるライブウィンドウ範囲を示す。
    let invalid = channel_with(&[("720p", &[101, 102, 103]), ("1080p", &[101, 102])]);
    let returned = refresh_snapshot(&mut snapshots, "live", &invalid, window()).unwrap();

    // 検証失敗により、以前に配信されたスナップショットは変更されない（SC-015）。
    assert_eq!(returned, &first);
    assert_eq!(snapshots.get("live").unwrap(), &first);
}

#[test]
fn refresh_surfaces_error_when_no_previous_snapshot_exists() {
    let mut snapshots: BTreeMap<String, ChannelSnapshot> = BTreeMap::new();
    let invalid = channel_with(&[("720p", &[101, 102, 103]), ("1080p", &[101, 102])]);
    assert!(refresh_snapshot(&mut snapshots, "live", &invalid, window()).is_err());
    assert!(!snapshots.contains_key("live"));
}
