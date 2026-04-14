use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Discriminator {
  pub column: String,
  pub values: HashMap<String, String>,
}

impl Discriminator {
  pub fn new(column: &str) -> Self {
    Self {
      column: column.to_string(),
      values: HashMap::new(),
    }
  }

  pub fn add_value(mut self, value: &str, entity_type: &str) -> Self {
    self
      .values
      .insert(value.to_string(), entity_type.to_string());
    self
  }

  pub fn get_entity_type(&self, value: &str) -> Option<&str> {
    self.values.get(value).map(|s| s.as_str())
  }
}

pub trait Discriminated {
  fn discriminator_column() -> &'static str;
  fn discriminator_value() -> &'static str;
}
