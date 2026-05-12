use crate::error::OrmResult;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ComputedField<E> {
  pub name: String,
  pub compute: Arc<dyn Fn(&E) -> OrmResult<Value> + Send + Sync>,
  pub cache: bool,
}

impl<E> ComputedField<E> {
  pub fn new(
    name: impl Into<String>,
    compute: impl Fn(&E) -> OrmResult<Value> + Send + Sync + 'static,
  ) -> Self {
    Self {
      name: name.into(),
      compute: Arc::new(compute),
      cache: false,
    }
  }

  pub fn cached(
    name: impl Into<String>,
    compute: impl Fn(&E) -> OrmResult<Value> + Send + Sync + 'static,
  ) -> Self {
    Self {
      name: name.into(),
      compute: Arc::new(compute),
      cache: true,
    }
  }

  pub fn evaluate(&self, entity: &E) -> OrmResult<Value> {
    (self.compute)(entity)
  }
}

impl<E> Clone for ComputedField<E> {
  fn clone(&self) -> Self {
    Self {
      name: self.name.clone(),
      compute: Arc::clone(&self.compute),
      cache: self.cache,
    }
  }
}

pub trait Accessors {
  type Entity;
  fn accessors() -> Vec<ComputedField<Self::Entity>>;
}

pub trait EntityAccessors: Sized {
  type Entity;
  fn accessors() -> Vec<ComputedField<Self::Entity>>;
}

pub struct CachedAccessor {
  value: Value,
  invalidated: bool,
}

impl CachedAccessor {
  pub fn new(value: Value) -> Self {
    Self {
      value,
      invalidated: false,
    }
  }

  pub fn get(&self) -> Option<&Value> {
    if self.invalidated {
      None
    } else {
      Some(&self.value)
    }
  }

  pub fn invalidate(&mut self) {
    self.invalidated = true;
  }

  pub fn update(&mut self, value: Value) {
    self.value = value;
    self.invalidated = false;
  }
}

pub struct AccessorsExecutor;

impl AccessorsExecutor {
  pub fn evaluate<E>(
    entity: &E,
    name: &str,
    accessors: &[ComputedField<E>],
  ) -> OrmResult<Option<Value>>
  where
    E: 'static,
  {
    for accessor in accessors {
      if accessor.name == name {
        return accessor.evaluate(entity).map(Some);
      }
    }
    Ok(None)
  }

  pub fn evaluate_all<E>(
    entity: &E,
    accessors: &[ComputedField<E>],
  ) -> OrmResult<HashMap<String, Value>>
  where
    E: 'static,
  {
    let mut results = HashMap::new();
    for accessor in accessors {
      results.insert(accessor.name.clone(), accessor.evaluate(entity)?);
    }
    Ok(results)
  }
}
