use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::query::{Filter, OrderBy, QueryBuilder, SortDirection};
use crate::relations::{RelationDef, RelationLoader, WithLoaded, WithRelations};
use crate::utils::generate_id;
use serde_json::Value;
use std::marker::PhantomData;

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
      _phantom: PhantomData,
    }
  }

  /// Returns the underlying provider (for raw access or sharing).
  pub fn provider(&self) -> &P {
    &self.provider
  }

  fn collection() -> String {
    E::table_name()
  }

  // ── Create / Update ──────────────────────────────────────────────────────

  /// Inserts a new entity (auto-generates an id if none is set).
  pub async fn insert(&self, mut entity: E) -> OrmResult<E> {
    if entity.get_id().is_none() {
      entity.set_id(generate_id());
    }
    let doc = entity.to_value()?;
    let stored = self.provider.insert(&Self::collection(), doc).await?;
    E::from_value(stored)
  }

  /// Updates an existing entity (must have an id).
  pub async fn update(&self, entity: E) -> OrmResult<E> {
    let id = entity
      .get_id()
      .ok_or_else(|| OrmError::InvalidQuery("Cannot update entity without an id".to_string()))?;
    let doc = entity.to_value()?;
    let stored = self.provider.update(&Self::collection(), &id, doc).await?;
    E::from_value(stored)
  }

  /// Inserts if no id, updates if id is present.
  pub async fn save(&self, entity: E) -> OrmResult<E> {
    if entity.get_id().is_some() {
      self.update(entity).await
    } else {
      self.insert(entity).await
    }
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
  pub async fn delete(&self, id: impl AsRef<str>) -> OrmResult<bool> {
    self.provider.delete(&Self::collection(), id.as_ref()).await
  }

  /// Delete an entity instance. Requires `get_id()` to return `Some`.
  pub async fn remove(&self, entity: &E) -> OrmResult<bool> {
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
  pub async fn soft_delete(&self, id: impl AsRef<str>) -> OrmResult<bool> {
    let patch = serde_json::json!({ "deleted_at": chrono::Utc::now() });
    self
      .provider
      .patch(&Self::collection(), id.as_ref(), patch)
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

  // ── Batch Operations ───────────────────────────────────────────────────────

  /// Batch insert multiple entities.
  /// Returns vector of inserted entities with generated IDs.
  pub async fn insert_many(&self, entities: Vec<E>) -> OrmResult<Vec<E>> {
    let mut results = Vec::with_capacity(entities.len());
    for entity in entities {
      results.push(self.insert(entity).await?);
    }
    Ok(results)
  }

  /// Batch update multiple entities by ID.
  /// Returns count of updated entities.
  pub async fn update_many(&self, entities: Vec<E>) -> OrmResult<u64> {
    let mut count = 0u64;
    for entity in entities {
      if entity.get_id().is_some() {
        self.update(entity).await?;
        count += 1;
      }
    }
    Ok(count)
  }

  /// Batch delete by IDs.
  /// Returns count of deleted entities.
  pub async fn delete_many(&self, ids: Vec<String>) -> OrmResult<u64> {
    let mut count = 0u64;
    for id in ids {
      if self.delete(&id).await? {
        count += 1;
      }
    }
    Ok(count)
  }

  /// Upsert many - insert or update based on presence of ID.
  pub async fn upsert_many(&self, entities: Vec<E>) -> OrmResult<Vec<E>> {
    let mut results = Vec::with_capacity(entities.len());
    for entity in entities {
      results.push(self.save(entity).await?);
    }
    Ok(results)
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

  /// Execute and return the first matching entity.
  pub async fn find_one(self) -> OrmResult<Option<E>> {
    Ok(self.limit(1).find().await?.into_iter().next())
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

  pub async fn insert(&self, entity: E) -> OrmResult<E> {
    self.inner.insert(entity).await
  }
  pub async fn update(&self, entity: E) -> OrmResult<E> {
    self.inner.update(entity).await
  }
  pub async fn save(&self, entity: E) -> OrmResult<E> {
    self.inner.save(entity).await
  }
  pub async fn delete(&self, id: impl AsRef<str>) -> OrmResult<bool> {
    self.inner.delete(id).await
  }

  /// Find by id and eagerly load the specified relations.
  pub async fn find_with_relations(
    &self,
    id: impl AsRef<str>,
    relation_names: &[&str],
  ) -> OrmResult<Option<WithLoaded<E>>> {
    let entity = match self.inner.find_by_id(id).await? {
      Some(e) => e,
      None => return Ok(None),
    };

    let all_rels = E::relations();
    let rels: Vec<RelationDef> = all_rels
      .into_iter()
      .filter(|r| relation_names.contains(&r.name.as_str()))
      .collect();

    let doc = entity.to_value()?;
    let loaded_map = self.loader.load(&doc, &rels).await?;

    let mut result = WithLoaded::new(entity);
    result.loaded = loaded_map;
    Ok(Some(result))
  }

  /// Find all entities and eagerly load specified relations for each.
  pub async fn find_all_with_relations(
    &self,
    relation_names: &[&str],
  ) -> OrmResult<Vec<WithLoaded<E>>> {
    let entities = self.inner.find_all().await?;
    self.hydrate(entities, relation_names).await
  }

  /// Run a query and eagerly load relations for the results.
  pub async fn query_with_relations(
    &self,
    builder: QueryBuilder,
    relation_names: &[&str],
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
    let entities: OrmResult<Vec<E>> = docs.into_iter().map(E::from_value).collect();
    self.hydrate(entities?, relation_names).await
  }

  async fn hydrate(
    &self,
    entities: Vec<E>,
    relation_names: &[&str],
  ) -> OrmResult<Vec<WithLoaded<E>>> {
    let all_rels = E::relations();
    let rels: Vec<RelationDef> = all_rels
      .into_iter()
      .filter(|r| relation_names.contains(&r.name.as_str()))
      .collect();

    let mut result = Vec::with_capacity(entities.len());
    for entity in entities {
      let doc = entity.to_value()?;
      let loaded_map = self.loader.load(&doc, &rels).await?;
      let mut wl = WithLoaded::new(entity);
      wl.loaded = loaded_map;
      result.push(wl);
    }
    Ok(result)
  }
}
