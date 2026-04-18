use crate::error::OrmResult;
use crate::nosql_index::{NosqlIndex, NosqlIndexInfo};
use crate::query::Filter;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// Configuration options for a provider.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
  /// For JSON: the base directory. For MongoDB: the connection URI.
  pub connection: String,
  /// Optional database/schema name.
  pub database: Option<String>,
  /// Optional extra key-value options.
  pub options: HashMap<String, String>,
}

impl ProviderConfig {
  pub fn new(connection: impl Into<String>) -> Self {
    Self {
      connection: connection.into(),
      database: None,
      options: HashMap::new(),
    }
  }

  pub fn with_database(mut self, db: impl Into<String>) -> Self {
    self.database = Some(db.into());
    self
  }

  pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.options.insert(key.into(), value.into());
    self
  }
}

/// The low-level provider abstraction.
///
/// Both `JsonProvider` and `MongoProvider` implement this trait.
/// All operations work on raw `serde_json::Value` so the provider
/// does not need to know about concrete entity types.
#[async_trait]
pub trait DatabaseProvider: Send + Sync + Clone + 'static {
  /// Insert a new document. Returns the stored document (with id populated).
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value>;

  /// Find a document by its id.
  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>>;

  /// Find all documents matching the filter. `None` = no filter.
  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<Value>>;

  /// Replace a document entirely. Returns the updated document.
  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value>;

  /// Merge (patch) fields of a document. Returns the updated document.
  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value>;

  /// Delete a document. Returns `true` if it existed.
  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool>;

  /// Count documents matching the filter.
  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64>;

  /// Check whether a document with the given id exists.
  async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self
      .find_by_id(collection, id)
      .await
      .map(|opt| opt.is_some())
  }

  /// Convenience: find many without any options.
  async fn find_all(&self, collection: &str) -> OrmResult<Vec<Value>> {
    self
      .find_many(collection, None, None, None, None, true)
      .await
  }

  // ── Index Management ────────────────────────────────────────────────────────

  /// Create an index on a collection.
  ///
  /// # Arguments
  /// * `collection` - Collection name.
  /// * `index` - The index definition.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// provider.create_index(
  ///     "users",
  ///     &NosqlIndex::single("email", 1).unique(true)
  /// ).await?;
  /// ```
  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()>;

  /// Drop an index by name.
  ///
  /// # Arguments
  /// * `collection` - Collection name.
  /// * `index_name` - Name of the index to drop.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// provider.drop_index("users", "idx_email").await?;
  /// ```
  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()>;

  /// List all indexes on a collection.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// let indexes = provider.list_indexes("users").await?;
  /// for idx in indexes {
  ///     println!("Index: {} - unique: {}", idx.name, idx.unique);
  /// }
  /// ```
  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<NosqlIndexInfo>>;

  /// Check if an index exists by name.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// if provider.index_exists("users", "idx_email").await? {
  ///     println!("Index exists!");
  /// }
  /// ```
  async fn index_exists(&self, collection: &str, index_name: &str) -> OrmResult<bool> {
    let indexes = self.list_indexes(collection).await?;
    Ok(indexes.iter().any(|i| i.name == index_name))
  }
}
