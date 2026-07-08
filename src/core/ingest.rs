//! セグメントインジェスト: 正規ストレージ、冪等性、メタデータ、コミットイベント。

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use crate::core::storage::CanonicalStore;
use crate::domain::rendition::RenditionMetadata;
use crate::domain::segment::{SegmentId, parse_filename, validate_identifier};
use crate::domain::window::{LiveWindow, WindowConfig};
use crate::error::EdamameError;

/// チャンネルごとのインジェスト状態。
#[derive(Debug, Default)]
pub struct ChannelState {
    pub renditions: BTreeMap<String, RenditionState>,
}

/// レンディションごとのコミット済みセグメント、メタデータ、コミットログ。
#[derive(Debug, Default)]
pub struct RenditionState {
    pub metadata: RenditionMetadata,
    pub segments: BTreeMap<u64, SegmentRecord>,
    pub commits: Vec<SegmentCommitEvent>,
}

impl RenditionState {
    /// 最も高いコミット済みシーケンス。つまり現在のライブエッジ。
    #[must_use]
    pub fn live_edge(&self) -> Option<u64> {
        self.segments.keys().next_back().copied()
    }
}

/// コミット済みセグメントの保存された拡張子とコンテンツフィンガープリント。
#[derive(Debug, Clone)]
pub struct SegmentRecord {
    pub ext: String,
    pub hash: u64,
}

/// セグメントがプレイリスト配信対象になったことを記録する（仕様 FR-004）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentCommitEvent {
    pub sequence: u64,
    pub committed_at_ms: u64,
}

/// インジェスト試行の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestOutcome {
    /// 新しいセグメントが保存されコミットされた。
    Created,
    /// 同一の識別子と同一のバイト列が既に存在する（冪等）。
    AlreadyExists,
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// 単一セグメントをインジェストし、識別子・メタデータ・冪等性ルールを強制する。
///
/// * 同一のバイト列の繰り返しは冪等。同一識別子に対する異なるバイト列は競合とする
///   （仕様 FR-030）。
/// * 競合するレンディションメタデータは拒否される（仕様 FR-041）。
#[allow(clippy::too_many_arguments)]
pub fn ingest_segment(
    channels: &mut BTreeMap<String, ChannelState>,
    store: &CanonicalStore,
    channel: &str,
    rendition: &str,
    filename: &str,
    metadata: &RenditionMetadata,
    bytes: &[u8],
    now_ms: u64,
) -> Result<IngestOutcome, EdamameError> {
    validate_identifier("channel", channel)?;
    validate_identifier("rendition", rendition)?;
    let (sequence, ext) = parse_filename(filename)?;

    let cstate = channels.entry(channel.to_owned()).or_default();
    let rstate = cstate.renditions.entry(rendition.to_owned()).or_default();

    rstate.metadata.merge(metadata)?;

    let hash = hash_bytes(bytes);
    if let Some(existing) = rstate.segments.get(&sequence) {
        if existing.hash == hash && existing.ext == ext {
            return Ok(IngestOutcome::AlreadyExists);
        }
        return Err(EdamameError::Conflict(format!(
            "segment {channel}/{rendition}/{sequence} already exists with different bytes"
        )));
    }

    store.write_segment(channel, rendition, sequence, &ext, bytes)?;
    rstate
        .segments
        .insert(sequence, SegmentRecord { ext, hash });
    rstate.commits.push(SegmentCommitEvent {
        sequence,
        committed_at_ms: now_ms,
    });
    Ok(IngestOutcome::Created)
}

/// キャッシュノードが ACTIVE になるために保持しなければならないライブウィンドウセグメント識別子の和集合。
#[must_use]
pub fn required_segments(
    channels: &BTreeMap<String, ChannelState>,
    window: WindowConfig,
) -> BTreeSet<SegmentId> {
    let mut required = BTreeSet::new();
    for (channel, cstate) in channels {
        for (rendition, rstate) in &cstate.renditions {
            let Some(edge) = rstate.live_edge() else {
                continue;
            };
            let live = LiveWindow {
                live_edge: edge,
                config: window,
            };
            for sequence in live.live_range() {
                if rstate.segments.contains_key(&sequence) {
                    required.insert(SegmentId {
                        channel: channel.clone(),
                        rendition: rendition.clone(),
                        sequence,
                    });
                }
            }
        }
    }
    required
}
