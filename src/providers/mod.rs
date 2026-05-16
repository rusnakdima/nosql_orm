use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use serde_json::Value;
use std::time::Duration;

pub struct FallbackProvider<P1: DatabaseProvider, P2: DatabaseProvider> {
  pub primary: P1,
  pub fallback: P2,
}

impl<P1: DatabaseProvider + Send + Sync, P2: DatabaseProvider + Send + Sync>
  FallbackProvider<P1, P2>
{
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
        eprintln!("Primary provider error: {}", e);
      }
      Err(_) => {
        eprintln!("Primary provider timeout");
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
        eprintln!("Primary provider error: {}", e);
      }
      Err(_) => {
        eprintln!("Primary provider timeout");
      }
    }

    self
      .fallback
      .find_many(collection, Some(filter), skip, limit, None, true)
      .await
  }
}

pub mod json;
pub mod sync;

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

#[cfg(feature = "clickhouse")]
pub mod clickhouse;
#[cfg(feature = "cockroach")]
pub mod cockroach;
#[cfg(feature = "dynamodb")]
pub mod dynamo;

#[cfg(feature = "clickhouse")]
pub use clickhouse::ClickHouseProvider;
#[cfg(feature = "cockroach")]
pub use cockroach::CockroachProvider;
#[cfg(feature = "dynamodb")]
pub use dynamo::DynamoProvider;
