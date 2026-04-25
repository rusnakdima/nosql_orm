use crate::error::OrmResult;
use crate::nosql_index::NosqlIndex;
use crate::sql::SqlColumnDef;
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
  /// SQL column definitions (for SQL providers).
  pub sql_columns: Option<Vec<SqlColumnDef>>,
}

impl EntityMeta {
  pub fn new(table_name: impl Into<String>) -> Self {
    Self {
      table_name: table_name.into(),
      id_field: "id".to_string(),
      sql_columns: None,
    }
  }

  pub fn with_id_field(mut self, field: impl Into<String>) -> Self {
    self.id_field = field.into();
    self
  }

  pub fn with_sql_columns(mut self, columns: Vec<SqlColumnDef>) -> Self {
    self.sql_columns = Some(columns);
    self
  }

  pub fn sql_table_name(&self) -> String {
    self.table_name.clone()
  }
}

pub trait FrontendProjection: Entity {
  fn frontend_excluded_fields() -> Vec<&'static str> {
    Vec::new()
  }

  fn filter_for_frontend(&self) -> Value {
    let mut value = self.to_value().unwrap_or(Value::Null);
    let excluded = Self::frontend_excluded_fields();
    if excluded.is_empty() {
      return value;
    }
    if let Some(obj) = value.as_object_mut() {
      for field in excluded {
        obj.remove(field);
      }
    }
    value
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

  /// Check if this entity supports soft deletes.
  fn is_soft_deletable() -> bool {
    false
  }

  /// Returns indexes defined for this entity.
  ///
  /// Override this method to define indexes on your entity.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// fn indexes() -> Vec<NosqlIndex> {
  ///     vec![
  ///         NosqlIndex::single("email", 1).unique(true).name("idx_email"),
  ///         NosqlIndex::ttl("created_at", 30 * 24 * 60 * 60).name("idx_ttl"),
  ///     ]
  /// }
  /// ```
  fn indexes() -> Vec<NosqlIndex> {
    Vec::new()
  }

  /// Returns SQL column definitions for this entity (used by SQL providers).
  ///
  /// Override this method to define SQL table schema.
  ///
  /// # Example
  ///
  /// ```rust,ignore
  /// fn sql_columns() -> Vec<SqlColumnDef> {
  ///     vec![
  ///         SqlColumnDef::new("id", SqlColumnType::Serial).primary_key(),
  ///         SqlColumnDef::new("name", SqlColumnType::VarChar(255)),
  ///         SqlColumnDef::new("email", SqlColumnType::VarChar(255)).unique(),
  ///     ]
  /// }
  /// ```
  fn sql_columns() -> Vec<SqlColumnDef> {
    Vec::new()
  }
}

/// A blanket helper: given a `Value` map, extract the string id.
pub fn extract_id(value: &Value, id_field: &str) -> Option<String> {
  value
    .get(id_field)
    .and_then(|v| v.as_str().map(ToString::to_string))
}
