use crate::error::OrmResult;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::fmt::Debug;

/// Metadata describing an entity (table/collection name, id field, etc.).
#[derive(Debug, Clone)]
pub struct EntityMeta {
  /// The table/collection name to store records in.
  pub table_name: String,
  /// The field used as primary key.
  pub id_field: String,
}

impl EntityMeta {
  pub fn new(table_name: impl Into<String>) -> Self {
    Self {
      table_name: table_name.into(),
      id_field: "id".to_string(),
    }
  }

  pub fn with_id_field(mut self, field: impl Into<String>) -> Self {
    self.id_field = field.into();
    self
  }
}

/// Core trait every ORM-managed struct must implement.
///
/// Typically derived via the `#[derive(Entity)]` macro (or implemented manually).
pub trait Entity: Serialize + DeserializeOwned + Debug + Clone + Send + Sync + 'static {
  /// Returns the metadata describing this entity.
  fn meta() -> EntityMeta;

  /// Returns the entity's primary-key value (as an `Option<String>`).
  fn get_id(&self) -> Option<String>;

  /// Sets the entity's primary-key value.
  fn set_id(&mut self, id: String);

  /// Serializes the entity to a `serde_json::Value`.
  fn to_value(&self) -> OrmResult<Value> {
    serde_json::to_value(self).map_err(Into::into)
  }

  /// Deserializes an entity from a `serde_json::Value`.
  fn from_value(value: Value) -> OrmResult<Self> {
    serde_json::from_value(value).map_err(Into::into)
  }

  /// Returns the table/collection name for this entity type.
  fn table_name() -> String {
    Self::meta().table_name
  }
}

/// A blanket helper: given a `Value` map, extract the string id.
pub fn extract_id(value: &Value, id_field: &str) -> Option<String> {
  value
    .get(id_field)
    .and_then(|v| v.as_str().map(ToString::to_string))
}
