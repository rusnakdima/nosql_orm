use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use super::types::{RelationValue, WithLoaded};

#[derive(Debug, Clone)]
pub struct OneToOne<T: Entity>(pub Option<T>);

#[derive(Debug, Clone)]
pub struct ManyToOne<T: Entity>(pub Option<T>);

#[derive(Debug, Clone)]
pub struct OneToMany<T: Entity>(pub Vec<T>);

#[derive(Debug, Clone)]
pub struct ManyToMany<T: Entity>(pub Vec<T>);

macro_rules! impl_relation_get {
  ($ty:ident, Option<&T>) => {
    impl<T: Entity> $ty<T> {
      pub fn get(&self) -> Option<&T> {
        self.0.as_ref()
      }
    }
  };
  ($ty:ident, &[T]) => {
    impl<T: Entity> $ty<T> {
      pub fn get(&self) -> &[T] {
        &self.0
      }
    }
  };
}

impl_relation_get!(OneToOne, Option<&T>);
impl_relation_get!(ManyToOne, Option<&T>);
impl_relation_get!(OneToMany, &[T]);
impl_relation_get!(ManyToMany, &[T]);

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

  pub fn keys(&self) -> Vec<&String> {
    self.loaded.keys().collect()
  }

  pub fn has(&self, name: &str) -> bool {
    self.loaded.contains_key(name)
  }
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
