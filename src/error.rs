/// Errors that can occur when operating on a graph.
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum GraphError {
    #[error("Node with ID {0} not found")]
    NodeNotFound(u64),

    #[error("Relationship with ID {0} not found")]
    RelationshipNotFound(u64),

    #[error("Property '{0}' not found")]
    PropertyNotFound(String),

    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        expected: &'static str,
        found: &'static str,
    },

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Quorum unavailable: {0}")]
    QuorumUnavailable(String),

    #[error("Not the leader. Current leader is {0:?}")]
    NotLeader(Option<u64>),

    #[error("Cluster error: {0}")]
    ClusterError(String),

    #[error("Storage I/O error: {0}")]
    Storage(String),

    #[error("Network error: {0}")]
    Network(String),
}

impl From<std::io::Error> for GraphError {
    fn from(err: std::io::Error) -> Self {
        GraphError::Storage(err.to_string())
    }
}

impl From<serde_json::Error> for GraphError {
    fn from(err: serde_json::Error) -> Self {
        GraphError::SerializationError(err.to_string())
    }
}

/// Convenience alias for `GraphError`.
pub use GraphError as Error;

/// Convenience alias for `Result<T, GraphError>`.
pub type Result<T> = std::result::Result<T, GraphError>;
