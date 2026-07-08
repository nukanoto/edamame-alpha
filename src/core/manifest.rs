//! 生成プレイリストの検証、アトミックなスナップショット置換、マニフェスト書き換え。

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::core::ingest::{ChannelState, RenditionState};
use crate::domain::playlist::{
    ChannelSnapshot, MasterPlaylist, MediaPlaylist, RenditionEntry, SegmentRef,
};
use crate::domain::window::{LiveWindow, WindowConfig};
use crate::error::EdamameError;

/// 1つのレンディションのメディアプレイリストを生成・検証する。
///
/// ライブウィンドウ内の欠落または未コミットのセグメントを参照する場合は、
/// プレイリストを保留する（`422`）（仕様 FR-003）。
pub fn generate_media_playlist(
    channel: &str,
    rendition: &str,
    rstate: &RenditionState,
    window: WindowConfig,
) -> Result<MediaPlaylist, EdamameError> {
    let edge = rstate.live_edge().ok_or_else(|| {
        EdamameError::Unprocessable(format!("{channel}/{rendition} has no committed segments"))
    })?;
    let live = LiveWindow {
        live_edge: edge,
        config: window,
    };

    let mut segments = Vec::new();
    for sequence in live.live_range() {
        if let Some(record) = rstate.segments.get(&sequence) {
            segments.push(SegmentRef {
                sequence,
                ext: record.ext.clone(),
            });
        }
    }

    let Some(first) = segments.first() else {
        return Err(EdamameError::Unprocessable(format!(
            "{channel}/{rendition} live window is empty"
        )));
    };
    let media_sequence = first.sequence;
    // ギャップを拒否: 生成されたセグメントはライブエッジまで連続していなければならない。
    let expected = edge - media_sequence + 1;
    if segments.len() as u64 != expected {
        return Err(EdamameError::Unprocessable(format!(
            "{channel}/{rendition} live window has a gap before the live edge"
        )));
    }

    Ok(MediaPlaylist {
        channel: channel.to_owned(),
        rendition: rendition.to_owned(),
        media_sequence,
        segments,
    })
}

/// チャンネル全体のスナップショットを再生成・検証する。
///
/// マスタープレイリストは、すべてのレンディションメディアプレイリストが有効で、
/// すべてのレンディションが同じライブウィンドウシーケンス範囲を持つ場合にのみ配信される
///（仕様 FR-024, FR-024a）。
pub fn regenerate(
    channel: &str,
    cstate: &ChannelState,
    window: WindowConfig,
) -> Result<ChannelSnapshot, EdamameError> {
    if cstate.renditions.is_empty() {
        return Err(EdamameError::NotFound(format!(
            "channel {channel} has no renditions"
        )));
    }

    let mut media = Vec::new();
    let mut entries = Vec::new();
    let mut range: Option<(u64, u64)> = None;

    for (rendition, rstate) in &cstate.renditions {
        let playlist = generate_media_playlist(channel, rendition, rstate, window)?;
        let playlist_range = playlist.live_range().ok_or_else(|| {
            EdamameError::Unprocessable(format!("{channel}/{rendition} live window is empty"))
        })?;
        match range {
            None => range = Some(playlist_range),
            Some(existing) if existing != playlist_range => {
                return Err(EdamameError::Unprocessable(
                    "renditions expose different live-window ranges".to_owned(),
                ));
            }
            _ => {}
        }
        entries.push(RenditionEntry {
            rendition: rendition.clone(),
            metadata: rstate.metadata.clone(),
        });
        media.push(playlist);
    }

    Ok(ChannelSnapshot {
        master: MasterPlaylist {
            channel: channel.to_owned(),
            renditions: entries,
        },
        media,
    })
}

/// チャンネルスナップショットを再生成し、成功時には保存済みスナップショットをアトミックに置き換える。
///
/// 検証失敗時には、以前保存されたスナップショットは変更されず、存在すれば返される。
/// 有効なスナップショットを一度も生成していないチャンネルのみがエラーを返す
///（仕様 FR-003a, SC-015）。
pub fn refresh_snapshot<'a>(
    snapshots: &'a mut BTreeMap<String, ChannelSnapshot>,
    channel: &str,
    cstate: &ChannelState,
    window: WindowConfig,
) -> Result<&'a ChannelSnapshot, EdamameError> {
    match regenerate(channel, cstate, window) {
        Ok(snapshot) => {
            snapshots.insert(channel.to_owned(), snapshot);
            Ok(snapshots
                .get(channel)
                .expect("snapshot was just inserted for channel"))
        }
        Err(err) => match snapshots.get(channel) {
            Some(existing) => Ok(existing),
            None => Err(err),
        },
    }
}

/// セグメントURLが単一の ACTIVE ノードを指すメディアプレイリストをレンダリングする
///（仕様 FR-031, FR-033）。
#[must_use]
pub fn render_media_m3u8(playlist: &MediaPlaylist, node_base_url: &str) -> String {
    let base = node_base_url.trim_end_matches('/');
    let mut out = String::new();
    out.push_str("#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:6\n");
    let _ = writeln!(out, "#EXT-X-MEDIA-SEQUENCE:{}", playlist.media_sequence);
    for segment in &playlist.segments {
        out.push_str("#EXTINF:6.0,\n");
        let _ = writeln!(
            out,
            "{base}/segments/{}/{}/{}.{}",
            playlist.channel, playlist.rendition, segment.sequence, segment.ext
        );
    }
    out
}

/// レンディションメディアプレイリストを参照するマスタープレイリストをレンダリングする（仕様 FR-025）。
#[must_use]
pub fn render_master_m3u8(master: &MasterPlaylist) -> String {
    let mut out = String::from("#EXTM3U\n#EXT-X-VERSION:3\n");
    for entry in &master.renditions {
        let bandwidth = entry.metadata.bandwidth.unwrap_or(1);
        let _ = write!(out, "#EXT-X-STREAM-INF:BANDWIDTH={bandwidth}");
        if let Some(resolution) = &entry.metadata.resolution {
            let _ = write!(out, ",RESOLUTION={resolution}");
        }
        out.push('\n');
        let _ = writeln!(out, "{}.m3u8", entry.rendition);
    }
    out
}
