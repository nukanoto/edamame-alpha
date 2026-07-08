//! オリジン責務のための正規ファイルシステムベースのセグメントストレージ。
//!
//! パスは [`std::path::Path`] の結合を使用し、プラットフォーム間でレイアウトが移植可能になるようにしている。

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::segment::{parse_filename, render_filename};
use crate::error::EdamameError;

/// ローカルディレクトリをルートとする正規ストア。
#[derive(Debug, Clone)]
pub struct CanonicalStore {
    root: PathBuf,
}

impl CanonicalStore {
    /// `root` をルートとするストアを作成する（初回書き込み時に遅延作成）。
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn rendition_dir(&self, channel: &str, rendition: &str) -> PathBuf {
        self.root.join(channel).join(rendition)
    }

    /// セグメントの正規バイト列を書き込み、必要に応じて親ディレクトリを作成する。
    pub fn write_segment(
        &self,
        channel: &str,
        rendition: &str,
        sequence: u64,
        ext: &str,
        bytes: &[u8],
    ) -> Result<(), EdamameError> {
        let dir = self.rendition_dir(channel, rendition);
        fs::create_dir_all(&dir)
            .map_err(|e| EdamameError::Internal(format!("create storage dir failed: {e}")))?;
        let path = dir.join(render_filename(sequence, ext));
        fs::write(&path, bytes)
            .map_err(|e| EdamameError::Internal(format!("write segment failed: {e}")))?;
        Ok(())
    }

    /// URLファイル名で識別されるセグメントの正規バイト列を読み込む。
    pub fn read_segment(
        &self,
        channel: &str,
        rendition: &str,
        filename: &str,
    ) -> Result<Vec<u8>, EdamameError> {
        let (sequence, ext) = parse_filename(filename)?;
        let path = self
            .rendition_dir(channel, rendition)
            .join(render_filename(sequence, &ext));
        read_path(&path)
    }
}

fn read_path(path: &Path) -> Result<Vec<u8>, EdamameError> {
    fs::read(path).map_err(|_| EdamameError::NotFound("segment not found".to_owned()))
}
