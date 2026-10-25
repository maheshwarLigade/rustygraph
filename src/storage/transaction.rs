use crate::cypher::{CypherResult, is_cypher_mutation};
use crate::error::{GraphError, Result};
use crate::graph::Graph;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Status of an ACID transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    Active,
    Committed,
    RolledBack,
    Expired,
}

/// An isolated multi-statement ACID transaction against the graph database.
pub struct Transaction {
    pub id: u64,
    pub created_at: Instant,
    pub timeout: Duration,
    pub snapshot_graph: Graph,
    pub mutating_queries: Vec<String>,
    pub status: TransactionStatus,
}

impl Transaction {
    /// Creates a new active transaction holding an isolated snapshot of the graph.
    pub fn new(id: u64, snapshot_graph: Graph, timeout: Duration) -> Self {
        Self {
            id,
            created_at: Instant::now(),
            timeout,
            snapshot_graph,
            mutating_queries: Vec::new(),
            status: TransactionStatus::Active,
        }
    }

    /// Checks if this transaction has exceeded its timeout duration.
    #[inline]
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.timeout
    }

    /// Returns the remaining duration before transaction timeout.
    #[inline]
    pub fn remaining_time(&self) -> Duration {
        self.timeout.saturating_sub(self.created_at.elapsed())
    }

    /// Executes an openCypher statement inside this isolated transaction snapshot.
    pub fn execute(&mut self, query_str: &str) -> Result<CypherResult> {
        if self.status != TransactionStatus::Active {
            return Err(GraphError::InvalidOperation(format!(
                "Transaction {} is not active (status: {:?})",
                self.id, self.status
            )));
        }

        if self.is_expired() {
            self.status = TransactionStatus::Expired;
            return Err(GraphError::InvalidOperation(format!(
                "Transaction {} timed out (configured timeout: {:?})",
                self.id, self.timeout
            )));
        }

        let res = self.snapshot_graph.cypher(query_str)?;

        if is_cypher_mutation(query_str) {
            self.mutating_queries.push(query_str.to_string());
        }

        Ok(res)
    }
}

/// Thread-safe transaction manager coordinating active multi-statement transactions,
/// isolation snapshots, and background timeout reaping.
pub struct TransactionManager {
    transactions: Arc<RwLock<HashMap<u64, Arc<Mutex<Transaction>>>>>,
    next_id: AtomicU64,
    default_timeout: Duration,
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

impl TransactionManager {
    /// Initializes a new TransactionManager with a specified default timeout.
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            default_timeout,
        }
    }

    /// Begins a new transaction with an isolated snapshot of the graph and timeout.
    pub fn begin(&self, snapshot: Graph, timeout: Option<Duration>) -> u64 {
        let tx_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let actual_timeout = timeout.unwrap_or(self.default_timeout);
        let tx = Transaction::new(tx_id, snapshot, actual_timeout);

        let mut map = self.transactions.write().unwrap();
        map.insert(tx_id, Arc::new(Mutex::new(tx)));
        tx_id
    }

    /// Executes a Cypher query within an active transaction.
    pub fn execute(&self, tx_id: u64, query_str: &str) -> Result<CypherResult> {
        let tx_arc = {
            let map = self.transactions.read().unwrap();
            map.get(&tx_id).cloned().ok_or_else(|| {
                GraphError::InvalidOperation(format!("Transaction {} not found", tx_id))
            })?
        };

        let mut tx = tx_arc.lock().unwrap();
        tx.execute(query_str)
    }

    /// Removes and unregisters a transaction from the active pool (used during commit or rollback).
    pub fn remove(&self, tx_id: u64) -> Result<Option<Transaction>> {
        let arc_opt = {
            let mut map = self.transactions.write().unwrap();
            map.remove(&tx_id)
        };

        match arc_opt {
            Some(arc) => {
                // Wait for any concurrent query execution to complete and acquire ownership
                let guard = arc.lock().unwrap();
                let owned = Transaction {
                    id: guard.id,
                    created_at: guard.created_at,
                    timeout: guard.timeout,
                    snapshot_graph: guard.snapshot_graph.clone(),
                    mutating_queries: guard.mutating_queries.clone(),
                    status: guard.status,
                };
                Ok(Some(owned))
            }
            None => Ok(None),
        }
    }

    /// Reaps expired transactions, removing them from active state and returning their IDs.
    pub fn reap_expired(&self) -> Vec<u64> {
        let mut map = self.transactions.write().unwrap();
        let mut expired_ids = Vec::new();

        map.retain(|&id, tx_arc| {
            if let Ok(tx) = tx_arc.try_lock() {
                if tx.is_expired() {
                    expired_ids.push(id);
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });

        expired_ids
    }

    /// Returns the number of currently open active transactions.
    pub fn active_count(&self) -> usize {
        self.transactions.read().unwrap().len()
    }

    /// Returns a list of active transaction metadata `(tx_id, remaining_ms, staged_mutations_count)`.
    pub fn list_active(&self) -> Vec<(u64, u64, usize)> {
        let map = self.transactions.read().unwrap();
        let mut list = Vec::new();
        for (&id, tx_arc) in map.iter() {
            if let Ok(tx) = tx_arc.try_lock() {
                list.push((
                    id,
                    tx.remaining_time().as_millis() as u64,
                    tx.mutating_queries.len(),
                ));
            }
        }
        list
    }
}
