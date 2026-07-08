//! 生成されたマスター/メディアプレイリストモデルと検証結果型。

use crate::domain::rendition::RenditionMetadata;

/// 生成されたメディアプレイリスト内の単一セグメントへの参照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentRef {
    pub sequence: u64,
    pub ext: String,
}

/// 単一レンディションの生成メディアプレイリスト。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPlaylist {
    pub channel: String,
    pub rendition: String,
    pub media_sequence: u64,
    pub segments: Vec<SegmentRef>,
}

impl MediaPlaylist {
    /// 空でない場合の、包含的な `(最初, 最後)` ライブウィンドウシーケンス範囲。
    #[must_use]
    pub fn live_range(&self) -> Option<(u64, u64)> {
        match (self.segments.first(), self.segments.last()) {
            (Some(first), Some(last)) => Some((first.sequence, last.sequence)),
            _ => None,
        }
    }
}

/// 生成されたマスタープレイリスト内の1つのレンディションエントリ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenditionEntry {
    pub rendition: String,
    pub metadata: RenditionMetadata,
}

/// 1つ以上のレンディションメディアプレイリストを参照する生成されたマスタープレイリスト。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterPlaylist {
    pub channel: String,
    pub renditions: Vec<RenditionEntry>,
}

/// 検証済みチャンネルスナップショット: マスタープレイリストと各レンディションメディアプレイリスト。
///
/// スナップショットは完全な検証が成功した後にのみ置換されるため、
/// 再生成が失敗しても以前に配信されたスナップショットは変更されない（仕様 FR-003a, SC-015）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSnapshot {
    pub master: MasterPlaylist,
    pub media: Vec<MediaPlaylist>,
}

impl ChannelSnapshot {
    /// 指定レンディションの生成メディアプレイリストを検索する。
    #[must_use]
    pub fn media_for(&self, rendition: &str) -> Option<&MediaPlaylist> {
        self.media.iter().find(|m| m.rendition == rendition)
    }
}
