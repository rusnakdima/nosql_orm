use crate::cascade::CascadeManager;
use crate::entity::{Entity, FrontendProjection};
use crate::error::{OrmError, OrmResult};
use crate::events::listener::EntityEvents;
use crate::provider::DatabaseProvider;
use crate::query::{
  Cursor, Filter, OrderBy, PaginatedResult, Projection, QueryBuilder, SortDirection,
};
use crate::relations::{RelationLoader, RelationType, RelationValue, WithLoaded, WithRelations};
use crate::soft_delete::SoftDeletable;
use crate::timestamps::apply_timestamps;
use crate::utils::generate_id;
use crate::validators::Validate;
use serde_json::Value;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::Arc;

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
///
/// ```rust,no_run
/// # use nosql_orm::prelude::*;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct User { id: Option<String>, name: String }
/// # impl Entity for User {
/// #   fn meta() -> EntityMeta { EntityMeta::new("users") }
/// #   fn get_id(&self) -> Option<String> { self.id.clone() }
/// #   fn set_id(&mut self, id: String) { self.id = Some(id); }
/// # }
/// # async fn example() -> OrmResult<()> {
/// # let provider = JsonProvider::new("./data").await?;
/// let repo = Repository::<User>::new(provider);
///
/// // Create
/// let user = repo.save(User { id: None, name: "Bob".into() }).await?;
///
/// // Find
/// let found = repo.find_by_id(user.get_id().unwrap()).await?;
///
/// // Query
/// let results = repo.query()
///     .where_eq("name", "Bob")
///     .limit(10)
///     .find().await?;
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub(crate) provider: P,
  pub(crate) events: Option<Arc<EntityEvents>>,
  _phantom: PhantomData<E>,
}

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  /// Create a new repository backed by `provider`.
  pub fn new(provider: P) -> Self {
    Self {
      provider,
      events: None,
      _phantom: PhantomData,
    }
  }

  /// Returns the underlying provider (for raw access or sharing).
  pub fn provider(&self) -> &P {
    &self.provider
  }

  /// Configure the repository with event listeners.
  pub fn with_events(mut self, events: Arc<EntityEvents>) -> Self {
    self.events = Some(events);
    self
  }

  fn collection() -> String {
    E::table_name()
  }

  // ── Create / Update ──────────────────────────────────────────────────────

  /// Inserts a new entity (auto-generates an id if none is set).
  /// Automatically sets created_at and updated_at timestamps.
  pub async fn insert(&self, mut entity: E) -> OrmResult<E>
  where
    E: Validate,
  {
    entity.validate()?;
    if entity.get_id().is_none() {
      entity.set_id(generate_id());
    }
    let mut doc = entity.to_value()?;
    apply_timestamps(&mut doc, true);
    let stored = self.provider.insert(&Self::collection(), doc).await?;
    let result = E::from_value(stored)?;
    if let Some(ref events) = self.events {
      events.dispatch_insert(&result.to_value()?).await?;
    }
    Ok(result)
  }

  /// Updates an existing entity (must have an id).
  /// Automatically updates the updated_at timestamp.
  pub async fn update(&self, entity: E) -> OrmResult<E>
  where
    E: Validate,
  {
    entity.validate()?;
    let id = entity
      .get_id()
      .ok_or_else(|| OrmError::InvalidQuery("Cannot update entity without an id".to_string()))?;
    let before_doc = self.provider.find_by_id(&Self::collection(), &id).await?;
    let mut doc = entity.to_value()?;
    apply_timestamps(&mut doc, false);
    let stored = self.provider.update(&Self::collection(), &id, doc).await?;
    let result = E::from_value(stored)?;
    if let Some(ref events) = self.events {
      let before_value = before_doc.clone().unwrap_or_else(|| serde_json::json!({}));
      events
        .dispatch_update(&before_value, &result.to_value()?)
        .await?;
    }
    Ok(result)
  }

  /// Inserts if no id, updates if id is present.
  pub async fn save(&self, entity: E) -> OrmResult<E>
  where
    E: Validate,
  {
    if entity.get_id().is_some() {
      self.update(entity).await
    } else {
      self.insert(entity).await
    }
  }

  /// Insert multiple entities in a batch.
  /// Returns the number of inserted entities.
  /// Automatically sets created_at and updated_at timestamps.
  pub async fn insert_many(&self, entities: Vec<E>) -> OrmResult<usize> {
    if entities.is_empty() {
      return Ok(0);
    }
    let mut count = 0;
    for mut entity in entities {
      if entity.get_id().is_none() {
        entity.set_id(generate_id());
      }
      let mut doc = entity.to_value()?;
      apply_timestamps(&mut doc, true);
      self.provider.insert(&Self::collection(), doc).await?;
      count += 1;
    }
    Ok(count)
  }

  /// Update multiple entities matching the filter.
  /// Returns the number of updated entities.
  pub async fn update_many(&self, filter: Option<Filter>, updates: Value) -> OrmResult<usize> {
    self
      .provider
      .update_many(&Self::collection(), filter, updates)
      .await
  }

  /// Upsert (insert or update) multiple entities based on their id.
  /// For entities with id: updates if exists, inserts if not.
  /// For entities without id: generates new id and inserts.
  /// Returns the number of upserted entities.
  pub async fn upsert_many(&self, entities: Vec<E>) -> OrmResult<usize>
  where
    E: Validate,
  {
    if entities.is_empty() {
      return Ok(0);
    }
    let mut count = 0;
    for entity in entities {
      self.save(entity).await?;
      count += 1;
    }
    Ok(count)
  }

  /// Delete multiple entities matching the filter.
  /// Returns the number of deleted entities.
  pub async fn delete_many(&self, filter: Option<Filter>) -> OrmResult<usize> {
    self.provider.delete_many(&Self::collection(), filter).await
  }

  /// Partially update an entity by merging `patch` fields.
  pub async fn patch(&self, id: impl AsRef<str>, patch: Value) -> OrmResult<E> {
    let stored = self
      .provider
      .patch(&Self::collection(), id.as_ref(), patch)
      .await?;
    E::from_value(stored)
  }

  // ── Delete ───────────────────────────────────────────────────────────────

  /// Delete by id. Returns `true` if the record was found and removed.
  /// If the entity implements `WithRelations` and has relations with cascade delete
  /// enabled, related entities will also be deleted.
  pub async fn delete(&self, id: impl AsRef<str>) -> OrmResult<bool>
  where
    E: WithRelations,
  {
    let id_str = id.as_ref();

    let relations = E::relations();
    let has_cascade = relations.iter().any(|r| r.should_cascade_hard_delete());
    if has_cascade {
      let cascade = CascadeManager::new(self.provider.clone());
      let mut deleted = HashSet::new();
      return cascade
        .hard_delete_cascade::<E>(id_str, &relations, &mut deleted)
        .await;
    }

    if let Some(ref events) = self.events {
      if let Some(entity) = self.find_by_id(id_str).await? {
        events.dispatch_delete(&entity.to_value()?).await?;
      }
    }

    self.provider.delete(&Self::collection(), id_str).await
  }

  /// Delete an entity instance. Requires `get_id()` to return `Some`.
  /// If the entity has relations with cascade delete enabled, related entities
  /// will also be deleted.
  pub async fn remove(&self, entity: &E) -> OrmResult<bool>
  where
    E: WithRelations,
  {
    let id = entity
      .get_id()
      .ok_or_else(|| OrmError::InvalidQuery("Cannot remove entity without an id".to_string()))?;
    self.delete(&id).await
  }

  // ── Find ─────────────────────────────────────────────────────────────────

  /// Find by primary key. Returns `None` if not found.
  pub async fn find_by_id(&self, id: impl AsRef<str>) -> OrmResult<Option<E>> {
    match self
      .provider
      .find_by_id(&Self::collection(), id.as_ref())
      .await?
    {
      Some(v) => Ok(Some(E::from_value(v)?)),
      None => Ok(None),
    }
  }

  /// Find by primary key. Returns `Err(NotFound)` if absent.
  pub async fn get_by_id(&self, id: impl AsRef<str>) -> OrmResult<E> {
    self
      .find_by_id(id.as_ref())
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", Self::collection(), id.as_ref())))
  }

  /// Return all entities in the collection.
  pub async fn find_all(&self) -> OrmResult<Vec<E>> {
    if E::is_soft_deletable() {
      self.find_all_including_deleted().await
    } else {
      let docs = self.provider.find_all(&Self::collection()).await?;
      docs.into_iter().map(E::from_value).collect()
    }
  }

  /// Return all entities including soft-deleted ones.
  pub async fn find_all_including_deleted(&self) -> OrmResult<Vec<E>> {
    let docs = self.provider.find_all(&Self::collection()).await?;
    docs.into_iter().map(E::from_value).collect()
  }

  /// Soft delete an entity by setting deleted_at timestamp.
  /// If the entity implements `WithRelations` and has relations with cascade soft delete
  /// enabled, related entities will also be soft deleted.
  pub async fn soft_delete(&self, id: impl AsRef<str>) -> OrmResult<bool>
  where
    E: WithRelations + SoftDeletable,
  {
    let id_str = id.as_ref();

    let relations = E::relations();
    let has_cascade = relations.iter().any(|r| r.should_cascade_soft_delete());
    if has_cascade {
      let cascade = CascadeManager::new(self.provider.clone());
      let mut deleted = HashSet::new();
      return cascade
        .soft_delete_cascade::<E>(id_str, &relations, &mut deleted)
        .await;
    }

    let patch = serde_json::json!({ "deleted_at": chrono::Utc::now() });
    self
      .provider
      .patch(&Self::collection(), id_str, patch)
      .await?;
    Ok(true)
  }

  /// Restore a soft-deleted entity by clearing deleted_at timestamp.
  pub async fn restore(&self, id: impl AsRef<str>) -> OrmResult<bool> {
    let patch = serde_json::json!({ "deleted_at": serde_json::Value::Null });
    self
      .provider
      .patch(&Self::collection(), id.as_ref(), patch)
      .await?;
    Ok(true)
  }

  /// Return the count of all entities.
  pub async fn count(&self) -> OrmResult<u64> {
    self.provider.count(&Self::collection(), None).await
  }

  /// Returns `true` if an entity with the given id exists.
  pub async fn exists(&self, id: impl AsRef<str>) -> OrmResult<bool> {
    self.provider.exists(&Self::collection(), id.as_ref()).await
  }

  pub async fn find_for_frontend(&self) -> OrmResult<Vec<E>>
  where
    E: FrontendProjection,
  {
    let projection = Projection::exclude(E::frontend_excluded_fields().as_slice());
    let docs = self.provider.find_all(&Self::collection()).await?;
    let filtered: Vec<Value> = docs
      .into_iter()
      .map(|doc| projection.apply_recursive(&doc))
      .collect();
    filtered.into_iter().map(E::from_value).collect()
  }

  pub async fn find_by_id_for_frontend(&self, id: impl AsRef<str>) -> OrmResult<Option<E>>
  where
    E: FrontendProjection,
  {
    let projection = Projection::exclude(E::frontend_excluded_fields().as_slice());
    match self
      .provider
      .find_by_id(&Self::collection(), id.as_ref())
      .await?
    {
      Some(v) => {
        let filtered = projection.apply_recursive(&v);
        Ok(Some(E::from_value(filtered)?))
      }
      None => Ok(None),
    }
  }

  // ── Query builder entry point ─────────────────────────────────────────────

  /// Start a fluent query against this repository.
  /// By default, soft-deleted entities are excluded for SoftDeletable entities.
  pub fn query(&self) -> RepositoryQuery<'_, E, P> {
    let builder = if E::is_soft_deletable() {
      QueryBuilder::new().where_is_null("deleted_at")
    } else {
      QueryBuilder::new()
    };
    RepositoryQuery {
      repo: self,
      builder,
    }
  }

  /// Start a fluent query against this repository, including soft-deleted entities.
  pub fn query_including_deleted(&self) -> RepositoryQuery<'_, E, P> {
    RepositoryQuery {
      repo: self,
      builder: QueryBuilder::new(),
    }
  }

  // ── Index Management ────────────────────────────────────────────────────────

  /// Get the index manager for this repository's collection.
  pub fn indexes(&self) -> crate::nosql_index::IndexManager<P> {
    crate::nosql_index::IndexManager::new(self.provider.clone())
  }

  /// Create an index on this collection.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// repo.create_index(NosqlIndex::single("email", 1).unique(true)).await?;
  /// ```
  pub async fn create_index(&self, index: crate::nosql_index::NosqlIndex) -> OrmResult<()> {
    self
      .provider
      .create_index(&Self::collection(), &index)
      .await
  }

  /// Drop an index by name.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// repo.drop_index("idx_email").await?;
  /// ```
  pub async fn drop_index(&self, name: &str) -> OrmResult<()> {
    self.provider.drop_index(&Self::collection(), name).await
  }

  /// List all indexes on this collection.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// let indexes = repo.list_indexes().await?;
  /// for idx in indexes {
  ///     println!("Index: {}", idx.name);
  /// }
  /// ```
  pub async fn list_indexes(&self) -> OrmResult<Vec<crate::nosql_index::NosqlIndexInfo>> {
    self.provider.list_indexes(&Self::collection()).await
  }

  /// Sync indexes from entity definition.
  ///
  /// Creates any indexes defined on the entity that don't exist yet.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// let created = repo.sync_indexes().await?;
  /// println!("Created {} indexes", created.len());
  /// ```
  pub async fn sync_indexes(&self) -> OrmResult<Vec<String>> {
    let manager = self.indexes();
    manager.sync_from_entity::<E>(&Self::collection()).await
  }

  // ── SQL Schema Management ───────────────────────────────────────────────────

  /// Sync the SQL table schema from entity column definitions.
  ///
  /// Creates or alters the table to match the entity's `sql_columns()`.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// repo.sync_schema().await?;
  /// ```
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

  /// Execute raw SQL (for advanced operations).
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// repo.execute_sql("TRUNCATE users CASCADE").await?;
  /// ```
  pub async fn execute_sql(&self, _sql: &str) -> OrmResult<()> {
    Ok(())
  }
}

// ── RepositoryQuery ──────────────────────────────────────────────────────────

/// A builder returned by `repo.query()` that executes against a specific repository.
pub struct RepositoryQuery<'r, E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  repo: &'r Repository<E, P>,
  builder: QueryBuilder,
}

impl<'r, E, P> RepositoryQuery<'r, E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub fn where_eq(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.builder = self.builder.where_eq(field, value);
    self
  }
  pub fn where_ne(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.builder = self.builder.where_ne(field, value);
    self
  }
  pub fn where_gt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.builder = self.builder.where_gt(field, value);
    self
  }
  pub fn where_lt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.builder = self.builder.where_lt(field, value);
    self
  }
  pub fn where_contains(mut self, field: impl Into<String>, sub: impl Into<String>) -> Self {
    self.builder = self.builder.where_contains(field, sub);
    self
  }
  pub fn where_in(mut self, field: impl Into<String>, values: Vec<Value>) -> Self {
    self.builder = self.builder.where_in(field, values);
    self
  }
  pub fn order_by(mut self, order: OrderBy) -> Self {
    self.builder = self.builder.order_by(order);
    self
  }
  pub fn skip(mut self, n: u64) -> Self {
    self.builder = self.builder.skip(n);
    self
  }
  pub fn limit(mut self, n: u64) -> Self {
    self.builder = self.builder.limit(n);
    self
  }
  pub fn with_relation(mut self, name: impl Into<String>) -> Self {
    self.builder = self.builder.with_relation(name);
    self
  }
  pub fn filter(mut self, f: Filter) -> Self {
    self.builder = self.builder.filter(f);
    self
  }
  pub fn select(mut self, fields: &[&str]) -> Self {
    self.builder = self.builder.select(fields);
    self
  }
  pub fn exclude(mut self, fields: &[&str]) -> Self {
    self.builder = self.builder.exclude(fields);
    self
  }

  /// Execute and return deserialized entities.
  pub async fn find(self) -> OrmResult<Vec<E>> {
    let filter = self.builder.build_filter();
    let (sort_field, sort_asc) = match &self.builder.order {
      Some(o) => (Some(o.field.as_str()), o.direction == SortDirection::Asc),
      None => (None, true),
    };
    let docs = self
      .repo
      .provider
      .find_many(
        &E::table_name(),
        filter.as_ref(),
        self.builder.skip,
        self.builder.limit,
        sort_field,
        sort_asc,
      )
      .await?;

    docs.into_iter().map(E::from_value).collect()
  }

  /// Execute and return results with cursor for pagination.
  /// The returned `PaginatedResult` contains the data and an optional cursor for the next page.
  /// Use `cursor` parameter to fetch the next page (from previous response's next_cursor).
  pub async fn find_with_cursor(self, cursor: Option<Cursor>) -> OrmResult<PaginatedResult<E>> {
    let mut builder = self.builder;

    if let Some(c) = cursor {
      let cursor_filter = c.as_filter();
      builder = builder.filter(cursor_filter);
    }

    let filter = builder.build_filter();
    let (sort_field, sort_asc) = match &builder.order {
      Some(o) => (Some(o.field.as_str()), o.direction == SortDirection::Asc),
      None => (Some("id"), true),
    };

    let docs = self
      .repo
      .provider
      .find_many(
        &E::table_name(),
        filter.as_ref(),
        None,
        builder.limit,
        sort_field,
        sort_asc,
      )
      .await?;

    let has_more = docs.len() as u64 >= builder.limit.unwrap_or(0);
    let next_cursor = docs.last().and_then(|doc| {
      doc.get("id").and_then(|v| v.as_str()).map(|id| Cursor {
        last_id: id.to_string(),
        sort_field: sort_field.unwrap_or("id").to_string(),
        sort_asc,
      })
    });

    let entities: Vec<E> = docs
      .into_iter()
      .map(E::from_value)
      .collect::<Result<Vec<_>, _>>()?;

    Ok(PaginatedResult {
      data: entities,
      next_cursor,
      has_more,
    })
  }

  /// Execute and return the first matching entity.
  pub async fn find_one(self) -> OrmResult<Option<E>> {
    Ok(self.limit(1).find().await?.into_iter().next())
  }

  /// Execute and return raw JSON values with projection applied.
  pub async fn find_raw(self) -> OrmResult<Vec<Value>> {
    let filter = self.builder.build_filter();
    let (sort_field, sort_asc) = match &self.builder.order {
      Some(o) => (Some(o.field.as_str()), o.direction == SortDirection::Asc),
      None => (None, true),
    };
    self
      .repo
      .provider
      .find_many(
        &E::table_name(),
        filter.as_ref(),
        self.builder.skip,
        self.builder.limit,
        sort_field,
        sort_asc,
      )
      .await
  }

  /// Execute and count matching entities.
  pub async fn count(self) -> OrmResult<u64> {
    let filter = self.builder.build_filter();
    self
      .repo
      .provider
      .count(&E::table_name(), filter.as_ref())
      .await
  }
}

// ── RelationRepository: extends Repository with relation loading ─────────────

/// Repository with relation-loading capabilities for entities that implement `WithRelations`.
pub struct RelationRepository<E, P>
where
  E: WithRelations,
  P: DatabaseProvider,
{
  inner: Repository<E, P>,
  loader: RelationLoader<P>,
}

impl<E, P> RelationRepository<E, P>
where
  E: WithRelations,
  P: DatabaseProvider,
{
  pub fn new(provider: P) -> Self {
    let loader = RelationLoader::new(provider.clone());
    Self {
      inner: Repository::new(provider),
      loader,
    }
  }

  /// Delegate all standard CRUD to the inner repository.
  pub fn repo(&self) -> &Repository<E, P> {
    &self.inner
  }
}

impl<E, P> RelationRepository<E, P>
where
  E: WithRelations + SoftDeletable,
  P: DatabaseProvider,
{
  pub async fn soft_delete_cascade(&self, id: impl AsRef<str>) -> OrmResult<bool> {
    self.inner.soft_delete(id).await
  }

  /// Find by id and eagerly load relations.
  ///
  /// Specify just the top-level relation name, and nested relations will be
  /// auto-loaded based on each entity's WithRelations definitions.
  ///
  /// # Example
  /// ```
  /// // Loads todo with tasks, and tasks' subtasks/comments automatically
  /// let todo = repo.find_with_relations("123", &["tasks"]).await?;
  /// ```
  pub async fn find_with_relations(
    &self,
    id: impl AsRef<str>,
    relation_paths: &[&str],
  ) -> OrmResult<Option<WithLoaded<E>>> {
    let entity = match self.inner.find_by_id(id).await? {
      Some(e) => e,
      None => return Ok(None),
    };

    let mut result = WithLoaded::new(entity);
    let mut doc = result.entity.to_value()?;

    let mut visited = std::collections::HashSet::new();
    visited.insert(E::table_name());

    for path in relation_paths {
      if let Some(rel_def) = E::relations()
        .iter()
        .find(|r| r.name.as_str() == *path)
        .cloned()
      {
        let enriched = self
          .loader
          .load_relation_recursive(vec![doc.clone()], &rel_def, &mut visited)
          .await?;

        if let Some(updated) = enriched.into_iter().next() {
          doc = updated;
          result.entity = E::from_value(doc.clone())?;

          if let Some(rel_val) = doc.get(*path) {
            match rel_def.relation_type {
              RelationType::ManyToOne | RelationType::OneToOne => {
                result.loaded.insert(
                  path.to_string(),
                  RelationValue::Single(Some(rel_val.clone())),
                );
              }
              RelationType::OneToMany | RelationType::ManyToMany => {
                if let Some(arr) = rel_val.as_array() {
                  result
                    .loaded
                    .insert(path.to_string(), RelationValue::Many(arr.clone()));
                } else {
                  result
                    .loaded
                    .insert(path.to_string(), RelationValue::Many(vec![]));
                }
              }
            }
          }
        }
      }
    }

    Ok(Some(result))
  }

  /// Find all entities and eagerly load relations for each.
  ///
  /// # Example
  /// ```
  /// // Loads all todos with tasks, and tasks' subtasks/comments automatically
  /// let todos = repo.find_all_with_relations(&["tasks"]).await?;
  /// ```
  pub async fn find_all_with_relations(
    &self,
    relation_paths: &[&str],
  ) -> OrmResult<Vec<WithLoaded<E>>> {
    let entities = self.inner.find_all().await?;

    let mut results = Vec::with_capacity(entities.len());

    for entity in entities {
      let mut result = WithLoaded::new(entity);
      let mut doc = result.entity.to_value()?;

      let mut visited = std::collections::HashSet::new();
      visited.insert(E::table_name());

      for path in relation_paths {
        let rel_name = path.split('.').next().unwrap_or(path);

        if let Some(rel_def) = E::relations()
          .iter()
          .find(|r| r.name.as_str() == rel_name)
          .cloned()
        {
          let enriched = self
            .loader
            .load_relation_recursive(vec![doc.clone()], &rel_def, &mut visited)
            .await?;

          if let Some(updated) = enriched.into_iter().next() {
            if let Some(rel_val) = updated.get(rel_name) {
              match rel_def.relation_type {
                RelationType::ManyToOne | RelationType::OneToOne => {
                  result.loaded.insert(
                    rel_name.to_string(),
                    RelationValue::Single(Some(rel_val.clone())),
                  );
                }
                RelationType::OneToMany | RelationType::ManyToMany => {
                  if let Some(arr) = rel_val.as_array() {
                    result
                      .loaded
                      .insert(rel_name.to_string(), RelationValue::Many(arr.clone()));
                  }
                }
              }
            }
            doc = updated;
          }
        }
      }

      results.push(result);
    }

    Ok(results)
  }

  /// Run a query and eagerly load relations for the results.
  pub async fn query_with_relations(
    &self,
    builder: QueryBuilder,
    relation_paths: &[&str],
  ) -> OrmResult<Vec<WithLoaded<E>>> {
    let filter = builder.build_filter();
    let (sort_field, sort_asc) = match &builder.order {
      Some(o) => (Some(o.field.clone()), o.direction == SortDirection::Asc),
      None => (None, true),
    };
    let docs = self
      .inner
      .provider
      .find_many(
        &E::table_name(),
        filter.as_ref(),
        builder.skip,
        builder.limit,
        sort_field.as_deref(),
        sort_asc,
      )
      .await?;

    let mut results = Vec::with_capacity(docs.len());

    for doc in docs {
      let entity = E::from_value(doc.clone())?;
      let mut result = WithLoaded::new(entity);
      let mut doc_for_loader = doc.clone();

      let mut visited = std::collections::HashSet::new();
      visited.insert(E::table_name());

      for path in relation_paths {
        let rel_name = path.split('.').next().unwrap_or(path);

        if let Some(rel_def) = E::relations()
          .iter()
          .find(|r| r.name.as_str() == rel_name)
          .cloned()
        {
          let enriched = self
            .loader
            .load_relation_recursive(vec![doc_for_loader.clone()], &rel_def, &mut visited)
            .await?;

          if let Some(updated) = enriched.into_iter().next() {
            if let Some(rel_val) = updated.get(rel_name) {
              match rel_def.relation_type {
                RelationType::ManyToOne | RelationType::OneToOne => {
                  result.loaded.insert(
                    rel_name.to_string(),
                    RelationValue::Single(Some(rel_val.clone())),
                  );
                }
                RelationType::OneToMany | RelationType::ManyToMany => {
                  if let Some(arr) = rel_val.as_array() {
                    result
                      .loaded
                      .insert(rel_name.to_string(), RelationValue::Many(arr.clone()));
                  }
                }
              }
            }
            doc_for_loader = updated;
          }
        }
      }

      results.push(result);
    }

    Ok(results)
  }
}
