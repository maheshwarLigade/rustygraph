use crate::error::{Error, Result};
use crate::value::PropertyValue;
use crate::vector::VectorSimilarity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// =============================================================================
// Zero-Dependency IEEE 802.3 CRC32 Implementation
// =============================================================================

const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Computes the IEEE 802.3 CRC32 checksum for a byte slice.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFFu32;
    for &b in data {
        let idx = ((crc ^ (b as u32)) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
    }
    !crc
}

// =============================================================================
// WAL Operations & Records
// =============================================================================

/// Atomic operations logged to the Write-Ahead Log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WalOp {
    CreateNode {
        id: u64,
        labels: Vec<String>,
        properties: BTreeMap<String, PropertyValue>,
    },
    DeleteNode {
        id: u64,
    },
    UpdateNodeProperties {
        id: u64,
        properties: BTreeMap<String, PropertyValue>,
    },
    CreateEdge {
        id: u64,
        source: u64,
        target: u64,
        rel_type: String,
        properties: BTreeMap<String, PropertyValue>,
    },
    DeleteEdge {
        id: u64,
    },
    UpdateEdgeProperties {
        id: u64,
        properties: BTreeMap<String, PropertyValue>,
    },
    CreateVectorIndex {
        name: String,
        label: String,
        prop_key: String,
        dimension: usize,
        similarity: VectorSimilarity,
    },
    CreateFulltextIndex {
        name: String,
        label: String,
        property_key: String,
    },
    CypherQuery {
        query: String,
    },
    TransactionCommit {
        tx_id: u64,
        queries: Vec<String>,
    },
}

/// A serialized entry in the Write-Ahead Log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalRecord {
    pub sequence: u64,
    pub timestamp: u64,
    pub op: WalOp,
    pub crc32: u32,
}

/// Policy for syncing write-ahead log entries to durable storage disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsyncPolicy {
    /// Fsync on every single write transaction (maximum durability, lowest latency tolerance).
    Always,
    /// Fsync periodically at fixed time intervals (balanced throughput and safety).
    Periodic(Duration),
    /// Rely on the operating system page cache (highest throughput, risk of lost trailing writes on hard crash).
    Never,
}

// =============================================================================
// WAL Manager
// =============================================================================

/// Thread-safe append-only Write-Ahead Log manager with crash recovery.
pub struct WalManager {
    dir: PathBuf,
    current_file: Mutex<Option<File>>,
    current_seq: Mutex<u64>,
    fsync_policy: FsyncPolicy,
    last_fsync: Mutex<Instant>,
}

impl WalManager {
    /// Opens or initializes a Write-Ahead Log directory.
    pub fn open<P: AsRef<Path>>(dir: P, policy: FsyncPolicy) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let wal_file_path = dir.join("wal.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wal_file_path)?;

        let mut max_seq = 0;
        // Scan existing log to find last sequence number
        if let Ok(records) = Self::read_records_from_path(&wal_file_path) {
            if let Some(last) = records.last() {
                max_seq = last.sequence;
            }
        }

        Ok(Self {
            dir,
            current_file: Mutex::new(Some(file)),
            current_seq: Mutex::new(max_seq),
            fsync_policy: policy,
            last_fsync: Mutex::new(Instant::now()),
        })
    }

    /// Returns the current maximum sequence number in the WAL.
    pub fn current_sequence(&self) -> u64 {
        *self.current_seq.lock().unwrap()
    }

    /// Appends an operation to the Write-Ahead Log and returns the assigned sequence number.
    pub fn append(&self, op: WalOp) -> Result<u64> {
        let mut seq_guard = self.current_seq.lock().unwrap();
        let next_seq = *seq_guard + 1;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Compute CRC32 over the serialized operation
        let op_json = serde_json::to_vec(&op)?;
        let checksum = crc32(&op_json);

        let record = WalRecord {
            sequence: next_seq,
            timestamp: now,
            op,
            crc32: checksum,
        };

        let mut line = serde_json::to_vec(&record)?;
        line.push(b'\n');

        let mut file_guard = self.current_file.lock().unwrap();
        if let Some(file) = file_guard.as_mut() {
            file.write_all(&line)?;

            // Apply fsync policy
            match self.fsync_policy {
                FsyncPolicy::Always => {
                    file.sync_data()?;
                }
                FsyncPolicy::Periodic(interval) => {
                    let mut last_sync = self.last_fsync.lock().unwrap();
                    if last_sync.elapsed() >= interval {
                        file.sync_data()?;
                        *last_sync = Instant::now();
                    }
                }
                FsyncPolicy::Never => {}
            }
        } else {
            return Err(Error::Storage("WAL file handle is not open".to_string()));
        }

        *seq_guard = next_seq;
        Ok(next_seq)
    }

    /// Explicitly flushes and syncs the WAL file to disk.
    pub fn flush(&self) -> Result<()> {
        let mut file_guard = self.current_file.lock().unwrap();
        if let Some(file) = file_guard.as_mut() {
            file.flush()?;
            file.sync_all()?;
        }
        Ok(())
    }

    /// Reads all valid records from the current WAL file, verifying CRC32 checksums.
    /// Trailing corrupted or partial records are detected and reported/skipped.
    pub fn read_all_records(&self) -> Result<Vec<WalRecord>> {
        let wal_file_path = self.dir.join("wal.log");
        Self::read_records_from_path(&wal_file_path)
    }

    /// Truncates the WAL after a successful snapshot checkpoint.
    pub fn truncate_up_to(&self, _seq: u64) -> Result<()> {
        let mut file_guard = self.current_file.lock().unwrap();
        let wal_file_path = self.dir.join("wal.log");

        // Close current handle
        *file_guard = None;

        // Truncate file cleanly
        let new_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&wal_file_path)?;

        *file_guard = Some(new_file);
        Ok(())
    }

    fn read_records_from_path(path: &Path) -> Result<Vec<WalRecord>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for line_res in reader.lines() {
            let line = match line_res {
                Ok(l) => l,
                Err(_) => break, // EOF or read issue
            };

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(rec) = serde_json::from_str::<WalRecord>(trimmed) {
                // Verify CRC32
                if let Ok(op_bytes) = serde_json::to_vec(&rec.op) {
                    if crc32(&op_bytes) == rec.crc32 {
                        records.push(rec);
                    }
                }
            }
        }

        Ok(records)
    }
}
