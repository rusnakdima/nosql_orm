use crate::entity::Entity;
use crate::error::OrmResult;
use crate::events::listener::EntityEvents;
use crate::provider::DatabaseProvider;
use std::marker::PhantomData;
use std::sync::Arc;

mod crud;
mod delete;
mod fast_path;
mod find;
mod indexes;
mod query;
mod relations;

#[allow(unused_imports)]
pub use crud::*;
#[allow(unused_imports)]
pub use delete::*;
#[allow(unused_imports)]
pub use fast_path::*;
#[allow(unused_imports)]
pub use find::*;
#[allow(unused_imports)]
pub use indexes::*;
pub use query::*;
pub use relations::*;

#[derive(Debug, Clone)]
pub struct SyncResult {
  pub synced_count: usize,
  pub skipped_count: usize,
  pub errors: Vec<String>,
}

impl SyncResult {
  pub fn new() -> Self {
    Self {
      synced_count: 0,
      skipped_count: 0,
      errors: Vec::new(),
    }
  }

  pub fn with_error(mut self, error: String) -> Self {
    self.errors.push(error);
    self
  }

  pub fn is_success(&self) -> bool {
    self.errors.is_empty()
  }
}

impl Default for SyncResult {
  fn default() -> Self {
    Self::new()
  }
}

/// Generic repository providing full CRUD for any `Entity`.
#[derive(Clone)]
pub struct Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub(crate) provider: P,
  pub(crate) events: Option<Arc<EntityEvents>>,
  pub(crate) query_timeout: Option<QueryTimeout>,
  _phantom: PhantomData<E>,
}

#[derive(Debug, Clone)]
pub struct QueryTimeout {
  pub timeout_ms: u64,
}

impl Default for QueryTimeout {
  fn default() -> Self {
    Self { timeout_ms: 30000 }
  }
}

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub fn new(provider: P) -> Self {
    Self {
      provider,
      events: None,
      query_timeout: None,
      _phantom: PhantomData,
    }
  }

  pub fn provider(&self) -> &P {
    &self.provider
  }

  pub fn with_events(mut self, events: Arc<EntityEvents>) -> Self {
    self.events = Some(events);
    self
  }

  pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
    self.query_timeout = Some(QueryTimeout { timeout_ms });
    self
  }

  fn collection() -> String {
    E::table_name()
  }

  pub fn indexes(&self) -> crate::nosql_index::IndexManager<P> {
    crate::nosql_index::IndexManager::new(self.provider.clone())
  }

  pub async fn sync_schema(&self) -> OrmResult<()> {
    let columns = E::sql_columns();
    if columns.is_empty() {
      return Ok(());
    }

    let table_name = Self::collection();

    let _create_sql = format!(
      "CREATE TABLE IF NOT EXISTS {} ({})",
      table_name,
      columns
        .iter()
        .map(|c| c.to_sql(crate::sql::SqlDialect::PostgreSQL))
        .collect::<Vec<_>>()
        .join(", ")
    );

    Ok(())
  }

  pub async fn execute_sql(&self, _sql: &str) -> OrmResult<()> {
    Ok(())
  }
}
