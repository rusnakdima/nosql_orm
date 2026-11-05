use crate::error::OrmResult;
use crate::provider::Storage;
use crate::query::Filter;
use serde_json::Value;
use std::time::Duration;

pub struct FallbackProvider<P1: Storage, P2: Storage> {
  pub primary: P1,
  pub fallback: P2,
}

impl<P1: Storage + Send + Sync, P2: Storage + Send + Sync> FallbackProvider<P1, P2> {
  pub async fn find_with_fallback(
    &self,
    collection: &str,
    filter: &Filter,
    timeout: Duration,
  ) -> OrmResult<Option<Value>> {
    match tokio::time::timeout(
      timeout,
      self
        .primary
        .find_many(collection, Some(filter), None, Some(1), None, true),
    )
    .await
    {
      Ok(Ok(ref results)) if !results.is_empty() => {
        return Ok(results.first().cloned());
      }
      Ok(Ok(_)) => {}
      Ok(Err(e)) => {
        tracing::warn!("Primary provider error: {}", e);
      }
      Err(_) => {
        tracing::warn!("Primary provider timeout");
      }
    }

    let results = self
      .fallback
      .find_many(collection, Some(filter), None, Some(1), None, true)
      .await?;
    Ok(results.into_iter().next())
  }

  pub async fn find_all_with_fallback(
    &self,
    collection: &str,
    filter: &Filter,
    skip: Option<u64>,
    limit: Option<u64>,
    timeout: Duration,
  ) -> OrmResult<Vec<Value>> {
    match tokio::time::timeout(
      timeout,
      self
        .primary
        .find_many(collection, Some(filter), skip, limit, None, true),
    )
    .await
    {
      Ok(Ok(results)) => {
        if !results.is_empty() {
          return Ok(results);
        }
      }
      Ok(Err(e)) => {
        tracing::warn!("Primary provider error: {}", e);
      }
      Err(_) => {
        tracing::warn!("Primary provider timeout");
      }
    }

    self
      .fallback
      .find_many(collection, Some(filter), skip, limit, None, true)
      .await
  }
}

#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "json")]
pub mod json_tree;
pub mod sync;
// The variant enum only has meaning when both of its arms exist.
#[cfg(all(feature = "json", feature = "mongo"))]
pub mod variant;

pub mod backend;
pub use backend::{connect_backend, connect_by_name, BackendKind, ConnectionParts};

#[cfg(feature = "mongo")]
pub mod mongo;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(any(
  feature = "sql-postgres",
  feature = "sql-sqlite",
  feature = "sql-mysql"
))]
pub mod sql;

pub use sync::{ConflictResolution, ProviderSync, SyncOptions};

#[cfg(feature = "json")]
pub use json::JsonProvider;

#[cfg(feature = "json")]
pub use json_tree::JsonTreeProvider;

#[cfg(feature = "mongo")]
pub use mongo::MongoProvider;

#[cfg(feature = "redis")]
pub use redis::RedisProvider;

#[cfg(feature = "sql-postgres")]
pub use sql::PostgresProvider;

#[cfg(feature = "sql-sqlite")]
pub use sql::SqliteProvider;

#[cfg(feature = "sql-mysql")]
pub use sql::MySqlProvider;
