use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::OrmResult;
use crate::provider::DatabaseProvider;

/// Migration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationMeta {
  pub version: i64,
  pub name: String,
  pub applied_at: Option<DateTime<Utc>>,
}

/// A single migration (up and down)
#[async_trait]
pub trait Migration<P: DatabaseProvider>: Send + Sync {
  /// Migration version (e.g., 1, 2, 20240101000000)
  fn version(&self) -> i64;

  /// Human-readable name
  fn name(&self) -> &str;

  /// Apply the migration (up)
  async fn up(&self, provider: &P) -> OrmResult<()>;

  /// Revert the migration (down)
  async fn down(&self, provider: &P) -> OrmResult<()>;
}

/// SQL-based migration for SQL databases
pub struct SqlMigration {
  pub version: i64,
  pub name: String,
  pub up_sql: String,
  pub down_sql: String,
}

impl SqlMigration {
  pub fn new(version: i64, name: &str, up_sql: &str, down_sql: &str) -> Self {
    Self {
      version,
      name: name.to_string(),
      up_sql: up_sql.to_string(),
      down_sql: down_sql.to_string(),
    }
  }
}

#[async_trait]
impl<P: DatabaseProvider> Migration<P> for SqlMigration {
  fn version(&self) -> i64 {
    self.version
  }

  fn name(&self) -> &str {
    &self.name
  }

  async fn up(&self, _provider: &P) -> OrmResult<()> {
    Ok(())
  }

  async fn down(&self, _provider: &P) -> OrmResult<()> {
    Ok(())
  }
}

/// JSON-based migration for document databases
pub struct JsonMigration {
  pub version: i64,
  pub name: String,
  pub up_script: serde_json::Value,
  pub down_script: serde_json::Value,
}

impl JsonMigration {
  pub fn new(
    version: i64,
    name: &str,
    up_script: serde_json::Value,
    down_script: serde_json::Value,
  ) -> Self {
    Self {
      version,
      name: name.to_string(),
      up_script,
      down_script,
    }
  }
}

#[async_trait]
impl<P: DatabaseProvider> Migration<P> for JsonMigration {
  fn version(&self) -> i64 {
    self.version
  }

  fn name(&self) -> &str {
    &self.name
  }

  async fn up(&self, _provider: &P) -> OrmResult<()> {
    Ok(())
  }

  async fn down(&self, _provider: &P) -> OrmResult<()> {
    Ok(())
  }
}
