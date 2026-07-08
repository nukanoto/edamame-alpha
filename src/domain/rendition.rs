//! レンディションメタデータと競合検証。
//!
//! レンディション識別子はレンディションパスセグメントです。オプションの `bandwidth` と
//! `resolution` は生成されたマスタープレイリストのメタデータであり、識別子を定義することは
//! ありません（仕様 FR-028）。一度確立された後、競合するメタデータは拒否されます（仕様 FR-041）。

use serde::{Deserialize, Serialize};

use crate::error::EdamameError;

/// レンディションのオプションのビットレート/解像度メタデータ。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenditionMetadata {
    pub bandwidth: Option<u64>,
    pub resolution: Option<String>,
}

impl RenditionMetadata {
    /// 両方の値が存在し、同一フィールドで異なる場合に true。
    #[must_use]
    pub fn conflicts_with(&self, other: &RenditionMetadata) -> bool {
        let bandwidth_conflict = matches!(
            (self.bandwidth, other.bandwidth),
            (Some(a), Some(b)) if a != b
        );
        let resolution_conflict = matches!(
            (&self.resolution, &other.resolution),
            (Some(a), Some(b)) if a != b
        );
        bandwidth_conflict || resolution_conflict
    }

    /// 新しく提供されたメタデータを確立されたレコードにマージする。
    ///
    /// 入力メタデータが既に確立されたものと競合する場合はエラーを返す。
    /// それ以外の場合、以前空だったフィールドを埋める。
    pub fn merge(&mut self, incoming: &RenditionMetadata) -> Result<(), EdamameError> {
        if self.conflicts_with(incoming) {
            return Err(EdamameError::Conflict(
                "rendition metadata conflicts with established values".to_owned(),
            ));
        }
        if self.bandwidth.is_none() {
            self.bandwidth = incoming.bandwidth;
        }
        if self.resolution.is_none() {
            self.resolution = incoming.resolution.clone();
        }
        Ok(())
    }
}
