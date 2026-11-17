use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::lazy::lazy_relation::{LazyMany, LazyRelation};
use crate::provider::Storage;
use crate::query::Filter;
use crate::relations::{RelationDef, RelationLoader, RelationType, WithRelations};
use crate::repository::Repository;
use serde_json::Value;
use std::sync::Arc;

pub struct LazyLoader;

impl LazyLoader {
  pub fn relation<E, P>(
    repo: Arc<Repository<E, P>>,
    relation: RelationDef,
    local_id: String,
  ) -> LazyRelation<E, P>
  where
    E: Entity,
    P: Storage,
  {
    LazyRelation::new(repo, relation, local_id)
  }

  pub fn many<E, P>(
    repo: Arc<Repository<E, P>>,
    relation: RelationDef,
    local_id: String,
  ) -> LazyMany<E, P>
  where
    E: Entity,
    P: Storage,
  {
    LazyMany::new(repo, relation, local_id)
  }
}

pub trait RepositoryLazyExt<E, P>
where
  E: Entity,
  P: Storage,
{
  fn lazy_relation(self: Arc<Self>, relation: RelationDef, local_id: String) -> LazyRelation<E, P>;
  fn lazy_many(self: Arc<Self>, relation: RelationDef, local_id: String) -> LazyMany<E, P>;
}

impl<E, P> RepositoryLazyExt<E, P> for Repository<E, P>
where
  E: Entity,
  P: Storage,
{
  fn lazy_relation(self: Arc<Self>, relation: RelationDef, local_id: String) -> LazyRelation<E, P> {
    LazyLoader::relation(self, relation, local_id)
  }

  fn lazy_many(self: Arc<Self>, relation: RelationDef, local_id: String) -> LazyMany<E, P> {
    LazyLoader::many(self, relation, local_id)
  }
}

fn local_id_from_doc(doc: &Value, key: &str) -> OrmResult<String> {
  doc
    .get(key)
    .and_then(|v| v.as_str())
    .map(String::from)
    .ok_or_else(|| OrmError::InvalidQuery(format!("Document carries no relation key {}", key)))
}

fn target_repo<E, P>(provider: &P) -> Arc<Repository<E, P>>
where
  E: Entity,
  P: Storage,
{
  Arc::new(Repository::new(provider.clone()))
}

fn single_handle<E, P>(
  provider: &P,
  doc: &Value,
  relation: &RelationDef,
) -> OrmResult<LazyRelation<E, P>>
where
  E: Entity,
  P: Storage,
{
  match relation.relation_type {
    RelationType::ManyToOne | RelationType::OneToOne => {}
    _ => {
      return Err(OrmError::InvalidQuery(format!(
        "Relation {} is not a single-value relation",
        relation.name
      )));
    }
  }
  let local_id = local_id_from_doc(doc, &relation.local_key)?;
  Ok(LazyRelation::new(
    target_repo(provider),
    relation.clone(),
    local_id,
  ))
}

fn collection_handle<E, P>(
  provider: &P,
  doc: &Value,
  relation: &RelationDef,
) -> OrmResult<LazyMany<E, P>>
where
  E: Entity,
  P: Storage,
{
  match relation.relation_type {
    RelationType::OneToMany => {
      let local_id = local_id_from_doc(doc, &relation.local_key)?;
      Ok(LazyMany::new(
        target_repo(provider),
        relation.clone(),
        local_id,
      ))
    }
    RelationType::ManyToMany => {
      let join_field = relation
        .join_field
        .clone()
        .unwrap_or_else(|| "ids".to_string());
      let ids: Vec<Value> = doc
        .get(&join_field)
        .and_then(|v| v.as_array())
        .map(|arr| {
          arr
            .iter()
            .filter_map(|item| item.as_str().map(|id| Value::String(id.to_string())))
            .collect()
        })
        .unwrap_or_default();
      if ids.is_empty() {
        return Err(OrmError::InvalidQuery(format!(
          "Document carries no {} array for relation {}",
          join_field, relation.name
        )));
      }
      let filter = Filter::In("id".to_string(), ids);
      Ok(LazyMany::new(target_repo(provider), relation.clone(), String::new()).with_filter(filter))
    }
    _ => Err(OrmError::InvalidQuery(format!(
      "Relation {} is not a collection relation",
      relation.name
    ))),
  }
}

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: Storage,
{
  pub fn lazy_find(&self, id: impl Into<String>) -> LazyRelation<E, P> {
    LazyRelation::by_id(Arc::new(self.fork()), id.into())
  }
}

impl<E, P> Repository<E, P>
where
  E: Entity + WithRelations,
  P: Storage,
{
  pub fn lazy_load<T>(&self, doc: &Value, relation_name: &str) -> OrmResult<LazyRelation<T, P>>
  where
    T: Entity,
  {
    let relation = self.resolve_relation(relation_name)?;
    single_handle(&self.provider, doc, &relation)
  }

  pub fn lazy_load_many<T>(&self, doc: &Value, relation_name: &str) -> OrmResult<LazyMany<T, P>>
  where
    T: Entity,
  {
    let relation = self.resolve_relation(relation_name)?;
    collection_handle(&self.provider, doc, &relation)
  }

  fn resolve_relation(&self, relation_name: &str) -> OrmResult<RelationDef> {
    E::relations()
      .into_iter()
      .find(|relation| relation.name == relation_name)
      .ok_or_else(|| {
        OrmError::InvalidQuery(format!(
          "Entity {} has no relation {}",
          E::table_name(),
          relation_name
        ))
      })
  }
}

impl<P: Storage> RelationLoader<P> {
  pub fn lazy_one<E>(&self, doc: &Value, relation: &RelationDef) -> OrmResult<LazyRelation<E, P>>
  where
    E: Entity,
  {
    single_handle(&self.provider, doc, relation)
  }

  pub fn lazy_many<E>(&self, doc: &Value, relation: &RelationDef) -> OrmResult<LazyMany<E, P>>
  where
    E: Entity,
  {
    collection_handle(&self.provider, doc, relation)
  }
}
