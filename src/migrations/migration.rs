use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::OrmResult;
use crate::provider::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationMeta {
  pub version: i64,
  pub name: String,
  pub applied_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait Migration<P: Storage>: Send + Sync {
  fn version(&self) -> i64;

  fn name(&self) -> &str;

  async fn up(&self, provider: &P) -> OrmResult<()>;

  async fn down(&self, provider: &P) -> OrmResult<()>;
}

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
impl<P: Storage> Migration<P> for SqlMigration {
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
impl<P: Storage> Migration<P> for JsonMigration {
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
