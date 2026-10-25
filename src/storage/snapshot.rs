use crate::error::Result;
use crate::graph::Graph;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Metadata stored alongside a database snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub sequence: u64,
    pub timestamp: u64,
    pub node_count: u64,
    pub edge_count: u64,
}

/// Snapshot manager responsible for atomic state checkpoints and recoveries.
pub struct SnapshotManager {
    dir: PathBuf,
    keep_snapshots: usize,
}

impl SnapshotManager {
    /// Initializes a snapshot manager for a given database directory.
    pub fn new<P: AsRef<Path>>(dir: P, keep_snapshots: usize) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            keep_snapshots: keep_snapshots.max(1),
        })
    }

    /// Saves an atomic snapshot of the given graph at the specified WAL sequence number.
    pub fn save_snapshot(&self, graph: &Graph, sequence: u64) -> Result<PathBuf> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let meta = SnapshotMetadata {
            sequence,
            timestamp: now,
            node_count: graph.node_count() as u64,
            edge_count: graph.edge_count() as u64,
        };

        let graph_json = graph.to_json()?;

        let final_path = self.dir.join(format!("snapshot_{:016}.db", sequence));
        let meta_path = self
            .dir
            .join(format!("snapshot_{:016}.meta.json", sequence));

        let tmp_path = self.dir.join(format!("snapshot_{:016}.tmp", sequence));
        let tmp_meta = self.dir.join(format!("snapshot_{:016}.meta.tmp", sequence));

        // Write temporary files
        fs::write(&tmp_path, graph_json)?;
        let meta_json = serde_json::to_string_pretty(&meta)?;
        fs::write(&tmp_meta, meta_json)?;

        // Atomic rename
        fs::rename(&tmp_path, &final_path)?;
        fs::rename(&tmp_meta, &meta_path)?;

        // Prune older snapshots
        self.prune_old_snapshots()?;

        Ok(final_path)
    }

    /// Loads the most recent valid snapshot from disk, if any exists.
    pub fn load_latest_snapshot(&self) -> Result<Option<(u64, Graph)>> {
        let mut snapshots = self.list_snapshots()?;
        if snapshots.is_empty() {
            return Ok(None);
        }

        // Sort descending by sequence number
        snapshots.sort_by(|a, b| b.sequence.cmp(&a.sequence));

        for snap in snapshots {
            let db_path = self.dir.join(format!("snapshot_{:016}.db", snap.sequence));
            if let Ok(content) = fs::read_to_string(&db_path) {
                if let Ok(graph) = Graph::from_json(&content) {
                    return Ok(Some((snap.sequence, graph)));
                }
            }
        }

        Ok(None)
    }

    /// Lists all detected snapshots in the directory.
    pub fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>> {
        let mut list = Vec::new();
        if !self.dir.exists() {
            return Ok(list);
        }

        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                if file_name.starts_with("snapshot_") && file_name.ends_with(".meta.json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(meta) = serde_json::from_str::<SnapshotMetadata>(&content) {
                            list.push(meta);
                        }
                    }
                }
            }
        }

        Ok(list)
    }

    fn prune_old_snapshots(&self) -> Result<()> {
        let mut list = self.list_snapshots()?;
        if list.len() <= self.keep_snapshots {
            return Ok(());
        }

        // Sort descending
        list.sort_by(|a, b| b.sequence.cmp(&a.sequence));

        // Delete all beyond keep count
        for old in &list[self.keep_snapshots..] {
            let db_path = self.dir.join(format!("snapshot_{:016}.db", old.sequence));
            let meta_path = self
                .dir
                .join(format!("snapshot_{:016}.meta.json", old.sequence));
            let _ = fs::remove_file(db_path);
            let _ = fs::remove_file(meta_path);
        }

        Ok(())
    }
}
