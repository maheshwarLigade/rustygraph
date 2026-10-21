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
}

/// Convenience alias for `Result<T, GraphError>`.
pub type Result<T> = std::result::Result<T, GraphError>;
