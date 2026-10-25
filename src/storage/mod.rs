//! Storage and persistence subsystem for RustyGraph.
//!
//! Provides high-performance generational memory arenas, append-only Write-Ahead Logging (WAL)
//! with CRC32 data integrity verification, point-in-time snapshot checkpointing,
//! snapshot-isolated ACID transactions, and the standalone database engine facade.

pub mod arena;
pub mod db;
pub mod snapshot;
pub mod transaction;
pub mod wal;

// Primary re-exports for ergonomic access
pub use arena::SlotArena;
pub use db::{DatabaseConfig, DatabaseStats, StandaloneDatabase};
pub use snapshot::{SnapshotManager, SnapshotMetadata};
pub use transaction::{Transaction, TransactionManager, TransactionStatus};
pub use wal::{FsyncPolicy, WalManager, WalOp, WalRecord, crc32};
