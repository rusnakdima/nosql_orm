use thiserror::Error;

/// Central error type for all ORM operations.
#[derive(Debug, Error)]
pub enum OrmError {
  #[error("Record not found: {0}")]
  NotFound(String),

  #[error("Duplicate record: {0}")]
  Duplicate(String),

  #[error("Serialization error: {0}")]
  Serialization(#[from] serde_json::Error),

  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),

  #[error("Provider error: {0}")]
  Provider(String),

  #[error("Not supported: {0}")]
  NotSupported(String),

  #[error("Relation error: {0}")]
  Relation(String),

  #[error("Invalid query: {0}")]
  InvalidQuery(String),

  #[error("Invalid input: {0}")]
  InvalidInput(String),

  #[error("Query error: {0}")]
  Query(String),

  #[error("Connection error: {0}")]
  Connection(String),

  #[error("Transaction error: {0}")]
  Transaction(String),

  #[error("Cascade delete restricted: cannot delete {entity} because it has related entities in relation '{relation}'")]
  CascadeRestricted { entity: String, relation: String },

  #[cfg(feature = "mongo")]
  #[error("MongoDB error: {0}")]
  Mongo(#[from] mongodb::error::Error),

  #[cfg(feature = "redis")]
  #[error("Redis error: {0}")]
  Redis(#[from] redis::RedisError),

  #[error("Validation error: {0}")]
  Validation(String),

  #[error("Internal error: {0}")]
  Internal(String),

  #[error("Query timeout: {0}")]
  Timeout(String),
}

impl OrmError {
  pub fn connection<E: std::error::Error>(e: E) -> Self {
    OrmError::Connection(e.to_string())
  }

  pub fn query<E: std::error::Error>(e: E) -> Self {
    OrmError::Query(e.to_string())
  }
}

#[inline]
pub fn map_err_connection<E: std::error::Error, T>(result: Result<T, E>) -> OrmResult<T> {
  result.map_err(OrmError::connection)
}

#[inline]
pub fn map_err_query<E: std::error::Error, T>(result: Result<T, E>) -> OrmResult<T> {
  result.map_err(OrmError::query)
}

/// Convenience alias for `Result<T, OrmError>`.
pub type OrmResult<T> = Result<T, OrmError>;
