//! ノードローカルの保持セグメントストレージ、配信、エビクション。

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::domain::segment::{SegmentId, parse_filename, render_filename};
use crate::domain::window::LiveWindow;
use crate::domain::window::WindowConfig;
use crate::error::EdamameError;
use crate::node::NodeState;

/// ノードバイナリで設定されていない場合に使用されるデフォルトの保持ウィンドウ。
pub const DEFAULT_RETENTION_WINDOW: u64 = 12;

/// ローカルディレクトリをルートとする保持セグメントストア。
#[derive(Debug)]
pub struct NodeCache {
    dir: PathBuf,
    retention_window: u64,
    retained: BTreeMap<SegmentId, String>,
}

impl NodeCache {
    /// `dir` をルートとし、指定された保持ウィンドウを持つノードキャッシュを作成する。
    #[must_use]
    pub fn new(dir: PathBuf, retention_window: u64) -> Self {
        Self {
            dir,
            retention_window,
            retained: BTreeMap::new(),
        }
    }

    fn path_for(&self, id: &SegmentId, ext: &str) -> PathBuf {
        self.dir
            .join(&id.channel)
            .join(&id.rendition)
            .join(render_filename(id.sequence, ext))
    }

    /// 保持セグメントを保存し、保持ウィンドウを超えたシーケンスをエビクションする。
    pub fn store(&mut self, id: SegmentId, ext: &str, bytes: &[u8]) -> Result<(), EdamameError> {
        let path = self.path_for(&id, ext);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| EdamameError::Internal(format!("create cache dir failed: {e}")))?;
        }
        fs::write(&path, bytes)
            .map_err(|e| EdamameError::Internal(format!("write cache segment failed: {e}")))?;
        self.retained.insert(id, ext.to_owned());
        self.evict();
        Ok(())
    }

    /// セグメントの保持バイト列を読み込む。ミス時は `None`。
    #[must_use]
    pub fn get(&self, channel: &str, rendition: &str, sequence: u64) -> Option<Vec<u8>> {
        let id = SegmentId {
            channel: channel.to_owned(),
            rendition: rendition.to_owned(),
            sequence,
        };
        let ext = self.retained.get(&id)?;
        fs::read(self.path_for(&id, ext)).ok()
    }

    /// このノードが現在保持している識別子。
    #[must_use]
    pub fn retained_ids(&self) -> Vec<SegmentId> {
        self.retained.keys().cloned().collect()
    }

    /// `live_edge - retention_window` より古いシーケンスをレンディションごとにエビクションする
    ///（仕様 FR-015）。エビクションされた識別子を返す。
    pub fn evict(&mut self) -> Vec<SegmentId> {
        let mut live_edges: BTreeMap<(String, String), u64> = BTreeMap::new();
        for id in self.retained.keys() {
            let key = (id.channel.clone(), id.rendition.clone());
            let entry = live_edges.entry(key).or_insert(id.sequence);
            *entry = (*entry).max(id.sequence);
        }

        let retention = self.retention_window;
        let evicted: Vec<SegmentId> = self
            .retained
            .keys()
            .filter(|id| {
                let edge = live_edges[&(id.channel.clone(), id.rendition.clone())];
                let window = LiveWindow {
                    live_edge: edge,
                    config: WindowConfig {
                        window_size: 1,
                        retention_window: retention,
                    },
                };
                window.is_evictable(id.sequence)
            })
            .cloned()
            .collect();

        for id in &evicted {
            if let Some(ext) = self.retained.remove(id) {
                let _ = fs::remove_file(self.path_for(id, &ext));
            }
        }
        evicted
    }
}

/// `GET /segments/{channel}/{rendition}/{filename}` — 保持セグメントを配信する。
pub async fn get_segment(
    State(state): State<Arc<NodeState>>,
    Path((channel, rendition, filename)): Path<(String, String, String)>,
) -> Result<Response, EdamameError> {
    let (sequence, _ext) = parse_filename(&filename)?;
    let cache = state.cache.lock().expect("node cache mutex poisoned");
    match cache.get(&channel, &rendition, sequence) {
        Some(bytes) => {
            Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response())
        }
        None => Err(EdamameError::NotFound("segment not retained".to_owned())),
    }
}
