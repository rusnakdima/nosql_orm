use crate::error::OrmResult;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum CastType {
  String,
  Integer,
  Float,
  Boolean,
  DateTime,
  Json,
}

pub struct MutatorDef {
  pub field: String,
  pub cast: CastType,
  pub getter: Option<Arc<dyn Fn(Value) -> OrmResult<Value> + Send + Sync>>,
  pub setter: Option<Arc<dyn Fn(Value) -> OrmResult<Value> + Send + Sync>>,
}

impl MutatorDef {
  pub fn new(field: impl Into<String>, cast: CastType) -> Self {
    Self {
      field: field.into(),
      cast,
      getter: None,
      setter: None,
    }
  }

  pub fn with_getter<F>(mut self, getter: F) -> Self
  where
    F: Fn(Value) -> OrmResult<Value> + Send + Sync + 'static,
  {
    self.getter = Some(Arc::new(getter));
    self
  }

  pub fn with_setter<F>(mut self, setter: F) -> Self
  where
    F: Fn(Value) -> OrmResult<Value> + Send + Sync + 'static,
  {
    self.setter = Some(Arc::new(setter));
    self
  }

  pub fn on_get<F>(field: impl Into<String>, cast: CastType, getter: F) -> Self
  where
    F: Fn(Value) -> OrmResult<Value> + Send + Sync + 'static,
  {
    Self::new(field, cast).with_getter(getter)
  }

  pub fn on_set<F>(field: impl Into<String>, cast: CastType, setter: F) -> Self
  where
    F: Fn(Value) -> OrmResult<Value> + Send + Sync + 'static,
  {
    Self::new(field, cast).with_setter(setter)
  }
}

pub trait Mutators {
  fn mutators() -> Vec<MutatorDef>;
}

pub trait EntityMutators: Sized {
  type Entity;
  fn mutators() -> Vec<MutatorDef>;
}

pub struct MutatorsExecutor;

impl MutatorsExecutor {
  pub fn apply_get<E: Mutators>(_entity: &E, field: &str, value: Value) -> OrmResult<Value> {
    for mutator in E::mutators() {
      if mutator.field == field {
        if let Some(getter) = &mutator.getter {
          return getter(value);
        }
      }
    }
    Ok(value)
  }

  pub fn apply_set<E: Mutators>(_entity: &E, field: &str, value: Value) -> OrmResult<Value> {
    for mutator in E::mutators() {
      if mutator.field == field {
        if let Some(setter) = &mutator.setter {
          return setter(value);
        }
      }
    }
    Ok(value)
  }

  pub fn cast_value(value: Value, cast: &CastType) -> OrmResult<Value> {
    match cast {
      CastType::String => {
        let s = serde_json::json!(value.as_str().unwrap_or(""));
        Ok(s)
      }
      CastType::Integer => {
        if let Some(n) = value.as_i64() {
          Ok(serde_json::json!(n))
        } else if let Some(n) = value.as_str().and_then(|s| s.parse::<i64>().ok()) {
          Ok(serde_json::json!(n))
        } else {
          Ok(serde_json::json!(0))
        }
      }
      CastType::Float => {
        if let Some(n) = value.as_f64() {
          Ok(serde_json::json!(n))
        } else if let Some(n) = value.as_str().and_then(|s| s.parse::<f64>().ok()) {
          Ok(serde_json::json!(n))
        } else {
          Ok(serde_json::json!(0.0))
        }
      }
      CastType::Boolean => {
        let b = value.as_bool().unwrap_or(false);
        Ok(serde_json::json!(b))
      }
      CastType::DateTime => Ok(value),
      CastType::Json => Ok(value),
    }
  }
}
