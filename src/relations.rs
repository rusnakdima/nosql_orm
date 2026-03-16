use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// The four standard relation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationType {
  OneToOne,
  OneToMany,
  ManyToOne,
  ManyToMany,
}

/// Describes one relation from an entity to another collection.
#[derive(Debug, Clone)]
pub struct RelationDef {
  /// Logical name used in `with_relation("name")`.
  pub name: String,
  pub relation_type: RelationType,
  /// The foreign collection name.
  pub target_collection: String,
  /// The key on the *owning* side (e.g. `"author_id"` on Post).
  pub local_key: String,
  /// The key on the *target* side (e.g. `"id"` on User).
  pub foreign_key: String,
  /// For ManyToMany: the join collection / field name (e.g. `"tag_ids"`).
  pub join_field: Option<String>,
}

impl RelationDef {
  /// Shorthand for a Many-To-One (the local doc holds `local_key` pointing at `foreign_key`).
  pub fn many_to_one(
    name: impl Into<String>,
    target_collection: impl Into<String>,
    local_key: impl Into<String>,
  ) -> Self {
    Self {
      name: name.into(),
      relation_type: RelationType::ManyToOne,
      target_collection: target_collection.into(),
      local_key: local_key.into(),
      foreign_key: "id".to_string(),
      join_field: None,
    }
  }

  /// Shorthand for a One-To-Many (the target docs hold `foreign_key` pointing at `local_key`).
  pub fn one_to_many(
    name: impl Into<String>,
    target_collection: impl Into<String>,
    foreign_key: impl Into<String>,
  ) -> Self {
    Self {
      name: name.into(),
      relation_type: RelationType::OneToMany,
      target_collection: target_collection.into(),
      local_key: "id".to_string(),
      foreign_key: foreign_key.into(),
      join_field: None,
    }
  }

  /// Shorthand for a One-To-One (like ManyToOne but semantically singular).
  pub fn one_to_one(
    name: impl Into<String>,
    target_collection: impl Into<String>,
    local_key: impl Into<String>,
  ) -> Self {
    Self {
      name: name.into(),
      relation_type: RelationType::OneToOne,
      target_collection: target_collection.into(),
      local_key: local_key.into(),
      foreign_key: "id".to_string(),
      join_field: None,
    }
  }

  /// Shorthand for Many-To-Many via an embedded array of ids.
  pub fn many_to_many(
    name: impl Into<String>,
    target_collection: impl Into<String>,
    join_field: impl Into<String>,
  ) -> Self {
    Self {
      name: name.into(),
      relation_type: RelationType::ManyToMany,
      target_collection: target_collection.into(),
      local_key: "id".to_string(),
      foreign_key: "id".to_string(),
      join_field: Some(join_field.into()),
    }
  }
}

/// Trait for entities that declare their relations.
pub trait WithRelations: Entity {
  fn relations() -> Vec<RelationDef> {
    vec![]
  }
}

/// A loaded entity with its relations eagerly populated.
#[derive(Debug, Clone)]
pub struct WithLoaded<E: Entity> {
  pub entity: E,
  pub loaded: HashMap<String, RelationValue>,
}

/// The value attached to a loaded relation.
#[derive(Debug, Clone)]
pub enum RelationValue {
  Single(Option<Value>),
  Many(Vec<Value>),
}

impl<E: Entity> WithLoaded<E> {
  pub fn new(entity: E) -> Self {
    Self {
      entity,
      loaded: HashMap::new(),
    }
  }

  /// Get a single-record relation (OneToOne, ManyToOne).
  pub fn one(&self, name: &str) -> OrmResult<Option<&Value>> {
    match self.loaded.get(name) {
      Some(RelationValue::Single(v)) => Ok(v.as_ref()),
      Some(RelationValue::Many(_)) => Err(OrmError::Relation(format!(
        "'{}' is a many relation, use `.many()`",
        name
      ))),
      None => Ok(None),
    }
  }

  /// Get a multi-record relation (OneToMany, ManyToMany).
  pub fn many(&self, name: &str) -> OrmResult<&[Value]> {
    match self.loaded.get(name) {
      Some(RelationValue::Many(v)) => Ok(v.as_slice()),
      Some(RelationValue::Single(_)) => Err(OrmError::Relation(format!(
        "'{}' is a single relation, use `.one()`",
        name
      ))),
      None => Ok(&[]),
    }
  }
}

/// Loads relations for a list of raw documents using a provider.
pub struct RelationLoader<P: DatabaseProvider> {
  provider: P,
}

impl<P: DatabaseProvider> RelationLoader<P> {
  pub fn new(provider: P) -> Self {
    Self { provider }
  }

  /// Load the specified relations into a document, returning a `HashMap<name, RelationValue>`.
  pub async fn load(
    &self,
    doc: &Value,
    relations: &[RelationDef],
  ) -> OrmResult<HashMap<String, RelationValue>> {
    let mut loaded = HashMap::new();

    for rel in relations {
      let value = self.load_one(doc, rel).await?;
      loaded.insert(rel.name.clone(), value);
    }

    Ok(loaded)
  }

  async fn load_one(&self, doc: &Value, rel: &RelationDef) -> OrmResult<RelationValue> {
    match rel.relation_type {
      RelationType::ManyToOne | RelationType::OneToOne => {
        let id_val = doc.get(&rel.local_key).and_then(|v| v.as_str());
        match id_val {
          None => Ok(RelationValue::Single(None)),
          Some(id) => {
            let found = self.provider.find_by_id(&rel.target_collection, id).await?;
            Ok(RelationValue::Single(found))
          }
        }
      }

      RelationType::OneToMany => {
        use crate::query::Filter;
        let local_id = doc.get(&rel.local_key).and_then(|v| v.as_str());
        match local_id {
          None => Ok(RelationValue::Many(vec![])),
          Some(id) => {
            let filter = Filter::Eq(rel.foreign_key.clone(), Value::String(id.to_string()));
            let docs = self
              .provider
              .find_many(
                &rel.target_collection,
                Some(&filter),
                None,
                None,
                None,
                true,
              )
              .await?;
            Ok(RelationValue::Many(docs))
          }
        }
      }

      RelationType::ManyToMany => {
        let join_field = rel.join_field.as_deref().unwrap_or("ids");
        let ids: Vec<&str> = doc
          .get(join_field)
          .and_then(|v| v.as_array())
          .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
          .unwrap_or_default();

        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
          if let Some(found) = self.provider.find_by_id(&rel.target_collection, id).await? {
            results.push(found);
          }
        }
        Ok(RelationValue::Many(results))
      }
    }
  }
}

// ── Type-safe relation wrappers ──────────────────────────────────────────────

/// Holds a lazily-resolved OneToOne relation.
#[derive(Debug, Clone)]
pub struct OneToOne<T: Entity>(pub Option<T>);

/// Holds a lazily-resolved ManyToOne relation.
#[derive(Debug, Clone)]
pub struct ManyToOne<T: Entity>(pub Option<T>);

/// Holds a lazily-resolved OneToMany relation.
#[derive(Debug, Clone)]
pub struct OneToMany<T: Entity>(pub Vec<T>);

/// Holds a lazily-resolved ManyToMany relation.
#[derive(Debug, Clone)]
pub struct ManyToMany<T: Entity>(pub Vec<T>);

impl<T: Entity> OneToOne<T> {
  pub fn get(&self) -> Option<&T> {
    self.0.as_ref()
  }
}
impl<T: Entity> ManyToOne<T> {
  pub fn get(&self) -> Option<&T> {
    self.0.as_ref()
  }
}
impl<T: Entity> OneToMany<T> {
  pub fn get(&self) -> &[T] {
    &self.0
  }
}
impl<T: Entity> ManyToMany<T> {
  pub fn get(&self) -> &[T] {
    &self.0
  }
}
