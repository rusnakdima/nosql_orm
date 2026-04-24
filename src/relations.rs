use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::sql::types::SqlOnDelete;
use serde::Serialize;
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
  /// For relations where local key is stored in an array field (e.g., `assignees: Vec<String>`).
  /// When set, the loader extracts IDs from this array field instead of `local_key`.
  pub local_key_in_array: Option<String>,
  /// When set, the loaded relation is transformed by looking up values in this map field
  /// from another collection. E.g., `assigneesProfiles` resolves `assignees` (user IDs) to profiles.
  pub transform_map_via: Option<TransformMapVia>,
  /// ON DELETE action for SQL foreign keys. Inferred from SqlForeignKey if present.
  pub on_delete: Option<SqlOnDelete>,
  /// Whether to cascade soft delete to related entities.
  pub cascade_soft_delete: bool,
  /// Whether to cascade hard delete to related entities.
  pub cascade_hard_delete: bool,
}

#[derive(Debug, Clone)]
pub struct TransformMapVia {
  /// The field on the loaded relation that contains the lookup key (e.g., "userId").
  pub lookup_key: String,
  /// The collection to query for the transformation (e.g., "profiles").
  pub source_collection: String,
  /// The field name in source_collection to match against (e.g., "id").
  pub source_key: String,
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
      local_key_in_array: None,
      transform_map_via: None,
      on_delete: None,
      cascade_soft_delete: false,
      cascade_hard_delete: false,
    }
  }

  /// Shorthand for a Many-To-One where the local key is stored in an array field.
  /// The loader will extract IDs from this array and resolve each to a target entity.
  pub fn many_to_one_array(
    name: impl Into<String>,
    target_collection: impl Into<String>,
    array_field: impl Into<String>,
  ) -> Self {
    let array_str = array_field.into();
    Self {
      name: name.into(),
      relation_type: RelationType::ManyToOne,
      target_collection: target_collection.into(),
      local_key: array_str.clone(),
      foreign_key: "id".to_string(),
      join_field: None,
      local_key_in_array: Some(array_str),
      transform_map_via: None,
      on_delete: None,
      cascade_soft_delete: false,
      cascade_hard_delete: false,
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
      local_key_in_array: None,
      transform_map_via: None,
      on_delete: None,
      cascade_soft_delete: false,
      cascade_hard_delete: false,
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
      local_key_in_array: None,
      transform_map_via: None,
      on_delete: None,
      cascade_soft_delete: false,
      cascade_hard_delete: false,
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
      local_key_in_array: None,
      transform_map_via: None,
      on_delete: None,
      cascade_soft_delete: false,
      cascade_hard_delete: false,
    }
  }

  /// Set a transformation mapping: after loading, transform each record by looking up
  /// values in `via_field` against another collection.
  pub fn transform_map(
    mut self,
    lookup_key: impl Into<String>,
    source_collection: impl Into<String>,
    source_key: impl Into<String>,
  ) -> Self {
    self.transform_map_via = Some(TransformMapVia {
      lookup_key: lookup_key.into(),
      source_collection: source_collection.into(),
      source_key: source_key.into(),
    });
    self
  }

  /// Set that the local key is stored in an array field (e.g., `assignees: Vec<String>`).
  /// The loader will extract IDs from this array instead of reading `local_key` directly.
  pub fn local_key_in_array(mut self, array_field: impl Into<String>) -> Self {
    self.local_key_in_array = Some(array_field.into());
    self
  }

  /// Set the ON DELETE action for this relation (used for SQL FK constraint generation).
  pub fn on_delete(mut self, action: SqlOnDelete) -> Self {
    self.on_delete = Some(action);
    self.apply_on_delete_action(action);
    self
  }

  /// Infer cascade settings from ON DELETE action.
  fn apply_on_delete_action(&mut self, action: SqlOnDelete) {
    match action {
      SqlOnDelete::Cascade => {
        self.cascade_hard_delete = true;
        self.cascade_soft_delete = true;
      }
      SqlOnDelete::Restrict => {
        // RESTRICT is checked before cascade, not automatically set
      }
      SqlOnDelete::SetNull | SqlOnDelete::SetDefault | SqlOnDelete::NoAction => {
        // These don't imply cascade
      }
    }
  }

  /// Returns true if soft delete should cascade through this relation.
  pub fn should_cascade_soft_delete(&self) -> bool {
    self.cascade_soft_delete
  }

  /// Returns true if hard delete should cascade through this relation.
  pub fn should_cascade_hard_delete(&self) -> bool {
    self.cascade_hard_delete
  }

  /// Returns true if delete should be restricted if related entities exist.
  pub fn should_restrict(&self) -> bool {
    self.on_delete == Some(SqlOnDelete::Restrict)
  }
}

/// Trait for entities that declare their relations.
pub trait WithRelations: Entity {
  fn relations() -> Vec<RelationDef> {
    vec![]
  }
}

/// Global registry mapping collection names to their relation definitions.
/// This allows dynamic relation resolution instead of hardcoded path matching.
use std::sync::RwLock;

static RELATION_REGISTRY: RwLock<Option<HashMap<String, Vec<RelationDef>>>> = RwLock::new(None);

/// Register relation definitions for a collection.
pub fn register_collection_relations(collection: &str, relations: Vec<RelationDef>) {
  let mut guard = RELATION_REGISTRY.write().unwrap();
  let registry = guard.get_or_insert_with(HashMap::new);
  registry.insert(collection.to_string(), relations);
}

/// Get all registered relations for a collection.
pub fn get_collection_relations(collection: &str) -> Option<Vec<RelationDef>> {
  let guard = RELATION_REGISTRY.read().unwrap();
  guard
    .as_ref()
    .and_then(|registry| registry.get(collection).cloned())
}

/// Get a specific relation definition by collection and relation name.
pub fn get_relation_def(collection: &str, relation_name: &str) -> Option<RelationDef> {
  let guard = RELATION_REGISTRY.read().unwrap();
  guard.as_ref().and_then(|registry| {
    registry
      .get(collection)
      .and_then(|relations| relations.iter().find(|r| r.name == relation_name).cloned())
  })
}

/// Clear all registered relations (useful for testing).
#[allow(dead_code)]
pub fn clear_relation_registry() {
  let mut guard = RELATION_REGISTRY.write().unwrap();
  *guard = None;
}

/// A loaded entity with its relations eagerly populated.
#[derive(Debug, Clone)]
pub struct WithLoaded<E: Entity> {
  pub entity: E,
  pub loaded: HashMap<String, RelationValue>,
}

impl<E: Entity> Serialize for WithLoaded<E> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(None)?;

    if let Ok(value) = self.entity.to_value() {
      if let Some(obj) = value.as_object() {
        for (k, v) in obj {
          map.serialize_entry(k, v)?;
        }
      }
    }

    for (key, rel_val) in &self.loaded {
      match rel_val {
        RelationValue::Single(Some(v)) => {
          map.serialize_entry(key, v)?;
        }
        RelationValue::Single(None) => {
          map.serialize_entry(key, &serde_json::Value::Null)?;
        }
        RelationValue::Many(arr) => {
          map.serialize_entry(key, arr)?;
        }
      }
    }

    map.end()
  }
}

/// The value attached to a loaded relation.
#[derive(Debug, Clone)]
pub enum RelationValue {
  Single(Option<Value>),
  Many(Vec<Value>),
}

impl Serialize for RelationValue {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match self {
      RelationValue::Single(Some(v)) => v.serialize(serializer),
      RelationValue::Single(None) => serializer.serialize_none(),
      RelationValue::Many(arr) => arr.serialize(serializer),
    }
  }
}

impl<E: Entity> WithLoaded<E> {
  pub fn new(entity: E) -> Self {
    Self {
      entity,
      loaded: HashMap::new(),
    }
  }

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

  pub fn get(&self, path: &str) -> Option<&RelationValue> {
    self.loaded.get(path)
  }

  /// Get all loaded relation keys for this entity.
  pub fn keys(&self) -> Vec<&String> {
    self.loaded.keys().collect()
  }

  /// Check if a relation was loaded.
  pub fn has(&self, name: &str) -> bool {
    self.loaded.contains_key(name)
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

  fn filter_not_deleted(docs: Vec<Value>) -> Vec<Value> {
    docs
      .into_iter()
      .filter(|d| match d.get("deleted_at") {
        Some(v) if v.is_null() => true,
        Some(v) if v.as_str().map_or(false, |s| s.is_empty()) => true,
        Some(_) => false,
        None => true,
      })
      .collect()
  }

  fn apply_filter(filter: Option<&crate::query::Filter>) -> Option<crate::query::Filter> {
    if let Some(f) = filter {
      Some(crate::query::Filter::And(vec![
        f.clone(),
        crate::query::Filter::Or(vec![
          crate::query::Filter::IsNull("deleted_at".to_string()),
          crate::query::Filter::Eq("deleted_at".to_string(), Value::String("".to_string())),
        ]),
      ]))
    } else {
      Some(crate::query::Filter::Or(vec![
        crate::query::Filter::IsNull("deleted_at".to_string()),
        crate::query::Filter::Eq("deleted_at".to_string(), Value::String("".to_string())),
      ]))
    }
  }

  /// Load relations for multiple parent documents in a single batch.
  ///
  /// This is much more efficient than loading relations one-by-one because it:
  /// 1. Collects all foreign keys from all parent documents
  /// 2. Fetches all related records in one query
  /// 3. Groups related records by foreign key
  /// 4. Attaches related records to each parent document
  pub async fn load_many(
    &self,
    mut docs: Vec<Value>,
    relation: &RelationDef,
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    match relation.relation_type {
      RelationType::ManyToOne | RelationType::OneToOne => {
        self
          .load_many_to_one(&mut docs, relation, filter_deleted)
          .await
      }
      RelationType::OneToMany => {
        self
          .load_one_to_many(&mut docs, relation, filter_deleted)
          .await
      }
      RelationType::ManyToMany => {
        self
          .load_many_to_many(&mut docs, relation, filter_deleted)
          .await
      }
    }
  }

  /// Load multiple relations for a single document.
  pub async fn load(
    &self,
    doc: &Value,
    relations: &[RelationDef],
    filter_deleted: bool,
  ) -> OrmResult<HashMap<String, RelationValue>> {
    let mut current_doc = doc.clone();
    let mut loaded = HashMap::new();

    for rel in relations {
      let result = self
        .load_many(vec![current_doc.clone()], rel, filter_deleted)
        .await?;
      if let Some(updated) = result.first() {
        if let Some(rel_val) = updated.get(&rel.name) {
          match rel.relation_type {
            RelationType::ManyToOne | RelationType::OneToOne => {
              loaded.insert(
                rel.name.clone(),
                RelationValue::Single(Some(rel_val.clone())),
              );
            }
            RelationType::OneToMany | RelationType::ManyToMany => {
              if let Some(arr) = rel_val.as_array() {
                loaded.insert(rel.name.clone(), RelationValue::Many(arr.clone()));
              }
            }
          }
        }
        current_doc = updated.clone();
      }
    }

    Ok(loaded)
  }

  /// Batch load ManyToOne relations (e.g., todo.userId -> user)
  async fn load_many_to_one(
    &self,
    docs: &mut [Value],
    relation: &RelationDef,
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    let target_field = &relation.local_key;

    let all_ids: Vec<String> = if relation.local_key_in_array.is_some() {
      let array_field = relation.local_key_in_array.as_ref().unwrap();
      let mut ids = Vec::new();
      for doc in docs.iter() {
        if let Some(arr) = doc.get(array_field).and_then(|v| v.as_array()) {
          for item in arr {
            if let Some(id) = item.as_str() {
              ids.push(id.to_string());
            }
          }
        }
      }
      ids
    } else {
      docs
        .iter()
        .filter_map(|d| {
          d.get(target_field)
            .and_then(|v| v.as_str())
            .map(String::from)
        })
        .collect()
    };

    if all_ids.is_empty() {
      return Ok(docs.to_vec());
    }

    let base_filter = crate::query::Filter::In(
      "id".to_string(),
      all_ids.iter().map(|s| Value::String(s.clone())).collect(),
    );

    let filter = if filter_deleted {
      Self::apply_filter(Some(&base_filter))
    } else {
      Some(base_filter)
    };

    let mut related_docs = self
      .provider
      .find_many(
        &relation.target_collection,
        filter.as_ref(),
        None,
        None,
        None,
        true,
      )
      .await?;

    if filter_deleted {
      related_docs = Self::filter_not_deleted(related_docs);
    }

    let related_map: HashMap<String, Value> = related_docs
      .into_iter()
      .filter_map(|d| {
        d.clone()
          .get("id")
          .and_then(|id| id.as_str())
          .map(|id| (id.to_string(), d))
      })
      .collect();

    for doc in docs.iter_mut() {
      if let Some(obj) = doc.as_object_mut() {
        if let Some(id) = obj.get(target_field).and_then(|v| v.as_str()) {
          if let Some(related) = related_map.get(id) {
            obj.insert(relation.name.clone(), related.clone());
          }
        } else if relation.local_key_in_array.is_some() {
          if let Some(arr) = obj.get(&relation.local_key).and_then(|v| v.as_array()) {
            let resolved: Vec<Value> = arr
              .iter()
              .filter_map(|item| item.as_str().and_then(|id| related_map.get(id).cloned()))
              .collect();
            obj.insert(relation.name.clone(), Value::Array(resolved));
          }
        }
      }
    }

    Ok(docs.to_vec())
  }

  /// Batch load OneToMany relations (e.g., todo.id -> tasks.todoId)
  async fn load_one_to_many(
    &self,
    docs: &mut [Value],
    relation: &RelationDef,
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    let source_key = "id";

    let parent_ids: Vec<String> = docs
      .iter()
      .filter_map(|d| d.get(source_key).and_then(|v| v.as_str()).map(String::from))
      .collect();

    if parent_ids.is_empty() {
      return Ok(docs.to_vec());
    }

    let base_filter = crate::query::Filter::In(
      relation.foreign_key.clone(),
      parent_ids
        .iter()
        .map(|s| Value::String(s.clone()))
        .collect(),
    );

    let filter = if filter_deleted {
      Self::apply_filter(Some(&base_filter))
    } else {
      Some(base_filter)
    };

    let mut related_docs = self
      .provider
      .find_many(
        &relation.target_collection,
        filter.as_ref(),
        None,
        None,
        None,
        true,
      )
      .await?;

    if filter_deleted {
      related_docs = Self::filter_not_deleted(related_docs);
    }

    let grouped: HashMap<String, Vec<Value>> = {
      let mut map = HashMap::new();
      for rel_doc in related_docs {
        if let Some(fk_val) = rel_doc.get(&relation.foreign_key).and_then(|v| v.as_str()) {
          map
            .entry(fk_val.to_string())
            .or_insert_with(Vec::new)
            .push(rel_doc);
        }
      }
      map
    };

    for doc in docs.iter_mut() {
      if let Some(obj) = doc.as_object_mut() {
        if let Some(parent_id) = obj.get(source_key).and_then(|v| v.as_str()) {
          let related = grouped.get(parent_id).cloned().unwrap_or_default();
          obj.insert(relation.name.clone(), Value::Array(related));
        }
      }
    }

    Ok(docs.to_vec())
  }

  /// Batch load ManyToMany relations (e.g., categories in todo)
  async fn load_many_to_many(
    &self,
    docs: &mut [Value],
    relation: &RelationDef,
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    let join_field = relation.join_field.as_deref().unwrap_or("ids");

    let all_ids: Vec<String> = {
      let mut ids = Vec::new();
      for doc in docs.iter() {
        if let Some(arr) = doc.get(join_field).and_then(|v| v.as_array()) {
          for item in arr {
            if let Some(id) = item.as_str() {
              ids.push(id.to_string());
            }
          }
        }
      }
      ids
    };

    if all_ids.is_empty() {
      return Ok(docs.to_vec());
    }

    let base_filter = crate::query::Filter::In(
      "id".to_string(),
      all_ids.iter().map(|s| Value::String(s.clone())).collect(),
    );

    let filter = if filter_deleted {
      Self::apply_filter(Some(&base_filter))
    } else {
      Some(base_filter)
    };

    let mut related_docs = self
      .provider
      .find_many(
        &relation.target_collection,
        filter.as_ref(),
        None,
        None,
        None,
        true,
      )
      .await?;

    if filter_deleted {
      related_docs = Self::filter_not_deleted(related_docs);
    }

    let related_map: HashMap<String, Value> = related_docs
      .into_iter()
      .filter_map(|d| {
        d.clone()
          .get("id")
          .and_then(|id| id.as_str())
          .map(|id| (id.to_string(), d))
      })
      .collect();

    for doc in docs.iter_mut() {
      if let Some(obj) = doc.as_object_mut() {
        if let Some(arr) = obj.get(join_field).and_then(|v| v.as_array()) {
          let resolved: Vec<Value> = arr
            .iter()
            .filter_map(|item| item.as_str().and_then(|id| related_map.get(id).cloned()))
            .collect();
          obj.insert(relation.name.clone(), Value::Array(resolved));
        }
      }
    }

    Ok(docs.to_vec())
  }

  /// Load a single relation path on a document
  pub async fn load_relation(&self, doc: &Value, relation: &RelationDef) -> OrmResult<Value> {
    match relation.relation_type {
      RelationType::ManyToOne | RelationType::OneToOne => {
        let id_val = doc.get(&relation.local_key).and_then(|v| v.as_str());
        match id_val {
          None => Ok(doc.clone()),
          Some(id) => {
            if let Some(found) = self
              .provider
              .find_by_id(&relation.target_collection, id)
              .await?
            {
              let mut result = doc.clone();
              if let Some(obj) = result.as_object_mut() {
                obj.insert(relation.name.clone(), found);
              }
              Ok(result)
            } else {
              Ok(doc.clone())
            }
          }
        }
      }
      RelationType::OneToMany => {
        let local_id = doc.get(&relation.local_key).and_then(|v| v.as_str());
        match local_id {
          None => Ok(doc.clone()),
          Some(id) => {
            let filter =
              crate::query::Filter::Eq(relation.foreign_key.clone(), Value::String(id.to_string()));
            let docs = self
              .provider
              .find_many(
                &relation.target_collection,
                Some(&filter),
                None,
                None,
                None,
                true,
              )
              .await?;
            let mut result = doc.clone();
            if let Some(obj) = result.as_object_mut() {
              obj.insert(relation.name.clone(), Value::Array(docs));
            }
            Ok(result)
          }
        }
      }
      RelationType::ManyToMany => {
        let join_field = relation.join_field.as_deref().unwrap_or("ids");
        let ids: Vec<&str> = doc
          .get(join_field)
          .and_then(|v| v.as_array())
          .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
          .unwrap_or_default();

        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
          if let Some(found) = self
            .provider
            .find_by_id(&relation.target_collection, id)
            .await?
          {
            results.push(found);
          }
        }
        let mut result = doc.clone();
        if let Some(obj) = result.as_object_mut() {
          obj.insert(relation.name.clone(), Value::Array(results));
        }
        Ok(result)
      }
    }
  }

  /// Load nested relations by dot-notation path with batch loading at each level.
  ///
  /// Example: load_nested(docs, ["tasks", "subtasks", "comments"], true)
  ///
  /// This method iteratively processes each level:
  /// 1. Loads the first relation level for all parent docs in one batch
  /// 2. For each remaining segment, loads and attaches nested relations
  ///
  /// Works for unlimited depth (N levels).
  pub async fn load_nested(
    &self,
    docs: Vec<Value>,
    path_segments: &[&str],
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    if path_segments.is_empty() {
      return Ok(docs);
    }

    let mut current_docs = docs;

    for (i, segment) in path_segments.iter().enumerate() {
      let rel_def = self.get_relation_def_for_path(&current_docs, segment)?;

      current_docs = self
        .load_many(current_docs, &rel_def, filter_deleted)
        .await?;

      if i + 1 < path_segments.len() {
        current_docs = self
          .propagate_nested_to_children_iterative(
            current_docs,
            segment,
            &path_segments[i + 1..],
            filter_deleted,
          )
          .await?;
      }
    }

    Ok(current_docs)
  }

  fn get_relation_def_for_path(&self, docs: &[Value], segment: &str) -> OrmResult<RelationDef> {
    if docs.is_empty() {
      return Err(OrmError::InvalidQuery(format!(
        "Cannot determine relation for '{}': no documents provided",
        segment
      )));
    }

    let first = &docs[0];
    let collection = first
      .get("_collection")
      .and_then(|v| v.as_str())
      .unwrap_or("");

    let target = match get_relation_def(collection, segment) {
      Some(def) => def,
      None => {
        return Err(OrmError::InvalidQuery(format!(
          "Unknown relation path: collection='{}', segment='{}'. Register relations using register_collection_relations().",
          collection, segment
        )));
      }
    };

    Ok(target)
  }

  async fn propagate_nested_to_children_iterative(
    &self,
    mut docs: Vec<Value>,
    parent_segment: &str,
    remaining_segments: &[&str],
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    if remaining_segments.is_empty() {
      return Ok(docs);
    }

    let parent_arr = docs
      .iter_mut()
      .filter_map(|d| d.get(parent_segment).and_then(|v| v.as_array()).cloned())
      .flatten()
      .collect::<Vec<Value>>();

    if parent_arr.is_empty() {
      return Ok(docs);
    }

    let mut children_to_process = parent_arr;
    let mut segment_index = 0;

    while segment_index < remaining_segments.len() {
      let segment = remaining_segments[segment_index];
      let rel_def = self.get_relation_def_for_path(&children_to_process, segment)?;

      children_to_process = self
        .load_many(children_to_process, &rel_def, filter_deleted)
        .await?;

      segment_index += 1;

      if segment_index < remaining_segments.len() {
        let next_segment = remaining_segments[segment_index];
        children_to_process = self.flatten_and_get_children(children_to_process, segment)?;

        let next_rel_def = self.get_relation_def_for_path(&children_to_process, next_segment)?;
        children_to_process = self
          .load_many(children_to_process, &next_rel_def, filter_deleted)
          .await?;
        segment_index += 1;

        if segment_index < remaining_segments.len() {
          children_to_process = self.flatten_and_get_children(children_to_process, next_segment)?;
        }
      }
    }

    let loaded_children = children_to_process;

    for doc in docs.iter_mut() {
      if let Some(obj) = doc.as_object_mut() {
        if let Some(arr) = obj.get_mut(parent_segment) {
          if let Some(arr_mut) = arr.as_array_mut() {
            let child_ids: Vec<String> = arr_mut
              .iter()
              .filter_map(|c| c.get("id").and_then(|v| v.as_str()).map(String::from))
              .collect();

            let matched: Vec<Value> = loaded_children
              .iter()
              .filter(|c| {
                c.get("id")
                  .and_then(|v| v.as_str())
                  .map(|id| child_ids.contains(&id.to_string()))
                  .unwrap_or(false)
              })
              .cloned()
              .collect();

            *arr_mut = matched;
          }
        }
      }
    }

    Ok(docs)
  }

  fn flatten_and_get_children(&self, docs: Vec<Value>, segment: &str) -> OrmResult<Vec<Value>> {
    let children: Vec<Value> = docs
      .into_iter()
      .filter_map(|mut d| {
        if let Some(obj) = d.as_object_mut() {
          if obj.get(segment).and_then(|v| v.as_array()).is_some() {
            obj.insert(
              "_collection".to_string(),
              Value::String(segment.to_string()),
            );
            return Some(d);
          }
        }
        None
      })
      .collect();
    Ok(children)
  }

  /// Batch load relations for raw JSON Value documents (not entity-typed).
  ///
  /// This is useful when you have already-retrieved documents and want to
  /// load relations on them without going through the typed Repository.
  ///
  /// # Arguments
  ///
  /// * `docs` - The parent documents to load relations onto
  /// * `table` - The table/collection name of the parent documents (used for relation lookup)
  /// * `paths` - Dot-notation relation paths to load (e.g., ["tasks.subtasks", "user"])
  /// * `filter_deleted` - Whether to filter out soft-deleted related entities
  ///
  /// # Returns
  ///
  /// Documents with all specified relations eagerly loaded
  pub async fn load_relations_on_docs(
    &self,
    mut docs: Vec<Value>,
    table: &str,
    paths: &[&str],
    filter_deleted: bool,
  ) -> OrmResult<Vec<Value>> {
    for path in paths {
      let segments: Vec<&str> = path.split('.').collect();

      for doc in docs.iter_mut() {
        if let Some(obj) = doc.as_object_mut() {
          obj.insert("_collection".to_string(), Value::String(table.to_string()));
        }
      }

      docs = self.load_nested(docs, &segments, filter_deleted).await?;

      for doc in docs.iter_mut() {
        if let Some(obj) = doc.as_object_mut() {
          obj.remove("_collection");
        }
      }
    }

    Ok(docs)
  }

  /// Load cascade nested relations for a single entity (as Value).
  ///
  /// Returns a HashMap with compound keys like "tasks", "tasks.subtasks", etc.
  pub async fn load_cascade_for_entity(
    &self,
    entity_doc: &Value,
    table: &str,
    path: &str,
    filter_deleted: bool,
  ) -> OrmResult<HashMap<String, RelationValue>> {
    let mut results = HashMap::new();
    let segments: Vec<&str> = path.split('.').collect();

    if segments.is_empty() {
      return Ok(results);
    }

    let first = segments[0];
    let rel_def = get_relation_def(table, first).ok_or_else(|| {
      OrmError::InvalidQuery(format!("Unknown relation '{}' on '{}'", first, table))
    })?;

    let mut doc_with_collection = entity_doc.clone();
    if let Some(obj) = doc_with_collection.as_object_mut() {
      obj.insert("_collection".to_string(), Value::String(table.to_string()));
    }

    let loaded = self
      .load(
        &doc_with_collection,
        std::slice::from_ref(&rel_def),
        filter_deleted,
      )
      .await?;

    if let Some(value) = loaded.get(first) {
      results.insert(first.to_string(), value.clone());

      if segments.len() > 1 {
        let related_docs: Vec<Value> = match value {
          RelationValue::Single(v) => v.as_ref().map(|v| vec![v.clone()]).unwrap_or_default(),
          RelationValue::Many(arr) => arr.clone(),
        };

        if !related_docs.is_empty() {
          let mut docs_with_meta = related_docs;
          for d in &mut docs_with_meta {
            if let Some(obj) = d.as_object_mut() {
              obj.insert(
                "_collection".to_string(),
                Value::String(rel_def.target_collection.clone()),
              );
            }
          }

          let nested_docs = self
            .load_nested(docs_with_meta, &segments[1..], filter_deleted)
            .await?;

          let mut level_docs: Vec<Vec<Value>> = vec![];

          for seg in &segments {
            let seg_docs: Vec<Value> = nested_docs
              .iter()
              .filter_map(|d| d.get(*seg as &str).and_then(|v| v.as_array()))
              .flatten()
              .cloned()
              .collect();

            level_docs.push(seg_docs.clone());
          }

          for (i, _) in segments.iter().enumerate().skip(1) {
            let mut prefix = String::new();
            for j in 0..=i {
              if j > 0 {
                prefix.push('.');
              }
              prefix.push_str(segments[j]);
            }

            if i < level_docs.len() && !level_docs[i].is_empty() {
              results.insert(prefix, RelationValue::Many(level_docs[i].clone()));
            }
          }
        }
      }
    }

    Ok(results)
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
