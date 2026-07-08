//! セグメント識別子とファイル名のパース。
//!
//! 正規識別子は `(channel, rendition, sequence)` です。URLファイル名は可読性のために
//! シーケンスと拡張子 (`{sequence}.{ext}`) をレンダリングしますが、識別子を定義することは
//! ありません（仕様 FR-027, FR-034）。

use serde::{Deserialize, Serialize};

use crate::error::EdamameError;

/// 正規の順序付きセグメント識別子。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SegmentId {
    pub channel: String,
    pub rendition: String,
    pub sequence: u64,
}

impl SegmentId {
    /// チャンネルとレンディションの識別子を検証しながらセグメント識別子を構築する。
    pub fn new(
        channel: impl Into<String>,
        rendition: impl Into<String>,
        sequence: u64,
    ) -> Result<Self, EdamameError> {
        let channel = channel.into();
        let rendition = rendition.into();
        validate_identifier("channel", &channel)?;
        validate_identifier("rendition", &rendition)?;
        Ok(Self {
            channel,
            rendition,
            sequence,
        })
    }
}

/// JSONペイロードで使用されるセグメント識別子のワイヤー表現。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentKey {
    pub channel_id: String,
    pub rendition: String,
    pub sequence: u64,
}

impl From<&SegmentId> for SegmentKey {
    fn from(id: &SegmentId) -> Self {
        Self {
            channel_id: id.channel.clone(),
            rendition: id.rendition.clone(),
            sequence: id.sequence,
        }
    }
}

impl SegmentKey {
    /// ワイヤー形式を検証済みの [`SegmentId`] に変換する。
    pub fn into_id(self) -> Result<SegmentId, EdamameError> {
        SegmentId::new(self.channel_id, self.rendition, self.sequence)
    }
}

/// 空の識別子やパス区切り文字を含む識別子を拒否する。
pub fn validate_identifier(kind: &str, value: &str) -> Result<(), EdamameError> {
    if value.is_empty() {
        return Err(EdamameError::BadRequest(format!(
            "{kind} must not be empty"
        )));
    }
    if value.contains('/') || value.contains('\\') {
        return Err(EdamameError::BadRequest(format!(
            "{kind} must not contain path separators: {value}"
        )));
    }
    Ok(())
}

/// `101.ts` のようなURLファイル名を `(シーケンス, 拡張子)` にパースする。
///
/// 拡張子は入力されたまま保持され、書き換え後のURLが `.ts`、`.m4s`、`.mp4` を
/// 維持できるようにしています（仕様 FR-033）。
pub fn parse_filename(filename: &str) -> Result<(u64, String), EdamameError> {
    let (stem, ext) = filename.rsplit_once('.').ok_or_else(|| {
        EdamameError::BadRequest(format!("filename must include an extension: {filename}"))
    })?;
    if ext.is_empty() {
        return Err(EdamameError::BadRequest(format!(
            "filename extension must not be empty: {filename}"
        )));
    }
    let sequence: u64 = stem.parse().map_err(|_| {
        EdamameError::BadRequest(format!("filename sequence must be a number: {filename}"))
    })?;
    Ok((sequence, ext.to_owned()))
}

/// シーケンスと拡張子から正規のURLファイル名をレンダリングする。
#[must_use]
pub fn render_filename(sequence: u64, ext: &str) -> String {
    format!("{sequence}.{ext}")
}
