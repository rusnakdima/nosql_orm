use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InheritanceType {
  SingleTable,
  ClassTable,
  ConcreteTable,
}

pub trait Inheritance: Send + Sync {
  fn inheritance_type() -> Option<InheritanceType>;
  fn discriminator_value() -> Option<DiscriminatorValue>;
  fn parent_entity() -> Option<&'static str>;
  fn child_entities() -> Vec<&'static str>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscriminatorValue(pub String);

impl DiscriminatorValue {
  pub fn new(value: &str) -> Self {
    Self(value.to_string())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildEntity;

#[macro_export]
macro_rules! entity_inheritance {
  (sti, $parent:ident, $child:ident, $discriminator:expr) => {
    impl Inheritance for $child {
      fn inheritance_type() -> Option<InheritanceType> {
        Some(InheritanceType::SingleTable)
      }

      fn discriminator_value() -> Option<DiscriminatorValue> {
        Some(DiscriminatorValue::new($discriminator))
      }

      fn parent_entity() -> Option<&'static str> {
        Some(stringify!($parent))
      }

      fn child_entities() -> Vec<&'static str> {
        Vec::new()
      }
    }

    impl $child {
      pub fn parent(&self) -> $parent {
        unimplemented!()
      }
    }
  };

  (parent, $entity:ident) => {
    impl Inheritance for $entity {
      fn inheritance_type() -> Option<InheritanceType> {
        None
      }

      fn discriminator_value() -> Option<DiscriminatorValue> {
        None
      }

      fn parent_entity() -> Option<&'static str> {
        None
      }

      fn child_entities() -> Vec<&'static str> {
        Vec::new()
      }
    }
  };
}
