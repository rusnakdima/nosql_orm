use crate::entity::Entity;
use crate::error::OrmResult;
use crate::events::listener::EntityEvents;
use crate::provider::{AdminCommands, DatabaseProvider};
use std::collections::VecDeque;
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

  pub async fn execute_sql(&self, sql: &str) -> OrmResult<usize>
  where
    P: AdminCommands,
  {
    let result = self.provider.execute_raw(sql, vec![]).await?;
    Ok(result.affected_rows as usize)
  }
}

pub trait QueryStreamTrait<E> {
  type Item;
  fn next(&mut self) -> impl std::future::Future<Output = Option<Self::Item>> + Send;
}

pub struct QueryStream<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  provider: P,
  collection: String,
  filter: Option<crate::query::Filter>,
  skip: Option<u64>,
  limit: Option<u64>,
  sort_by: Option<String>,
  sort_asc: bool,
  batch: Option<VecDeque<E>>,
  batch_index: usize,
  batch_size: u64,
}

impl<E, P> QueryStream<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub fn new(
    provider: P,
    collection: impl Into<String>,
    filter: Option<crate::query::Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<String>,
    sort_asc: bool,
  ) -> Self {
    Self {
      provider,
      collection: collection.into(),
      filter,
      skip,
      limit,
      sort_by,
      sort_asc,
      batch: None,
      batch_index: 0,
      batch_size: 100,
    }
  }

  pub fn with_limit(mut self, limit: u64) -> Self {
    self.limit = Some(limit);
    self
  }

  pub fn with_skip(mut self, skip: u64) -> Self {
    self.skip = Some(skip);
    self
  }

  pub fn with_batch_size(mut self, batch_size: u64) -> Self {
    self.batch_size = batch_size;
    self
  }

  async fn fetch_batch(&mut self) -> OrmResult<()> {
    let current_index = self.batch_index as u64;
    let remaining_limit = self
      .limit
      .map(|l| l - current_index)
      .unwrap_or(self.batch_size);
    let fetch_limit = remaining_limit.min(self.batch_size);

    let items = self
      .provider
      .find_many(
        &self.collection,
        self.filter.as_ref(),
        Some(current_index),
        if fetch_limit > 0 {
          Some(fetch_limit)
        } else {
          None
        },
        self.sort_by.as_deref(),
        self.sort_asc,
      )
      .await?;

    let entities: Vec<E> = items
      .into_iter()
      .map(|v| E::from_value(v))
      .collect::<Result<Vec<_>, _>>()?;

    self.batch = Some(entities.into());
    self.batch_index = 0;
    Ok(())
  }

  pub async fn next_item(&mut self) -> OrmResult<Option<E>> {
    if let Some(ref mut batch) = self.batch {
      if self.batch_index < batch.len() {
        let item = batch.pop_front().unwrap();
        self.batch_index += 1;
        return Ok(Some(item));
      }
    }

    if let Some(limit) = self.limit {
      if self.batch_index >= limit as usize {
        return Ok(None);
      }
    }

    self.fetch_batch().await?;

    if let Some(ref mut batch) = self.batch {
      if !batch.is_empty() {
        let item = batch.pop_front().unwrap();
        self.batch_index += 1;
        return Ok(Some(item));
      }
    }

    Ok(None)
  }
}

impl<E, P> QueryStreamTrait<E> for QueryStream<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  type Item = OrmResult<E>;

  async fn next(&mut self) -> Option<Self::Item> {
    self.next_item().await.ok().and_then(|v| v.map(Ok))
  }
}

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub async fn find_stream(
    &self,
    filter: Option<crate::query::Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<String>,
    sort_asc: bool,
  ) -> OrmResult<QueryStream<E, P>> {
    Ok(QueryStream::new(
      self.provider.clone(),
      Self::collection(),
      filter,
      skip,
      limit,
      sort_by,
      sort_asc,
    ))
  }

  pub async fn find_many_stream(
    &self,
    filter: Option<crate::query::Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
  ) -> OrmResult<QueryStream<E, P>> {
    self.find_stream(filter, skip, limit, None, true).await
  }
}
