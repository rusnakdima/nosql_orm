use crate::entity::Entity;
use crate::sql::types::SqlOnDelete;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

pub trait WithRelations: crate::entity::Entity {
  fn relations() -> Vec<RelationDef> {
    vec![]
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationType {
  OneToOne,
  OneToMany,
  ManyToOne,
  ManyToMany,
}

#[derive(Debug, Clone)]
pub struct TransformMapVia {
  pub lookup_key: String,
  pub source_collection: String,
  pub source_key: String,
}

#[derive(Debug, Clone)]
pub struct RelationDef {
  pub name: String,
  pub relation_type: RelationType,
  pub target_collection: String,
  pub local_key: String,
  pub foreign_key: String,
  pub join_field: Option<String>,
  pub local_key_in_array: Option<String>,
  pub transform_map_via: Option<TransformMapVia>,
  pub on_delete: Option<SqlOnDelete>,
  pub cascade_soft_delete: bool,
  pub cascade_hard_delete: bool,
}

impl RelationDef {
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

  pub fn local_key_in_array(mut self, array_field: impl Into<String>) -> Self {
    self.local_key_in_array = Some(array_field.into());
    self
  }

  pub fn on_delete(mut self, action: SqlOnDelete) -> Self {
    self.on_delete = Some(action);
    self.apply_on_delete_action(action);
    self
  }

  fn apply_on_delete_action(&mut self, action: SqlOnDelete) {
    match action {
      SqlOnDelete::Cascade => {
        self.cascade_hard_delete = true;
        self.cascade_soft_delete = true;
      }
      SqlOnDelete::Restrict => {}
      SqlOnDelete::SetNull | SqlOnDelete::SetDefault | SqlOnDelete::NoAction => {}
    }
  }

  pub fn should_cascade_soft_delete(&self) -> bool {
    self.cascade_soft_delete
  }

  pub fn should_cascade_hard_delete(&self) -> bool {
    self.cascade_hard_delete
  }

  pub fn should_restrict(&self) -> bool {
    self.on_delete == Some(SqlOnDelete::Restrict)
  }

  pub fn cascade_soft_delete(mut self, cascade: bool) -> Self {
    self.cascade_soft_delete = cascade;
    self
  }

  pub fn cascade_hard_delete(mut self, cascade: bool) -> Self {
    self.cascade_hard_delete = cascade;
    self
  }
}

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

#[derive(Debug, Clone)]
pub struct WithLoaded<E: Entity> {
  pub entity: E,
  pub loaded: HashMap<String, RelationValue>,
}
