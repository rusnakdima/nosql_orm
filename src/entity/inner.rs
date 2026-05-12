use crate::error::OrmResult;
use crate::field_meta::FieldMeta;
use crate::nosql_index::NosqlIndex;
use crate::sql::SqlColumnDef;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct EntityMeta {
  pub table_name: String,
  pub id_field: String,
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

pub trait Entity: Serialize + DeserializeOwned + Debug + Send + Sync + 'static {
  fn meta() -> EntityMeta;

  fn fields() -> Vec<FieldMeta> {
    Vec::new()
  }

  fn get_id(&self) -> Option<String>;

  fn set_id(&mut self, id: String);

  fn to_value(&self) -> OrmResult<Value> {
    serde_json::to_value(self).map_err(Into::into)
  }

  fn from_value(value: Value) -> OrmResult<Self> {
    serde_json::from_value(value).map_err(Into::into)
  }

  fn table_name() -> String {
    Self::meta().table_name
  }

  fn is_soft_deletable() -> bool {
    false
  }

  fn indexes() -> Vec<NosqlIndex> {
    Vec::new()
  }

  fn sql_columns() -> Vec<SqlColumnDef> {
    Vec::new()
  }
}

pub fn extract_id(value: &Value, id_field: &str) -> Option<String> {
  value
    .get(id_field)
    .and_then(|v| v.as_str().map(ToString::to_string))
}
