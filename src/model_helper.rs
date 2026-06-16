use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
  pub field: String,
  pub message: String,
}

pub trait ModelHelper: Sized {
  fn validate(&self) -> Vec<ValidationError>;
  fn before_insert(&mut self);
  fn before_update(&mut self);
  fn after_load(&mut self);
  fn transform(&mut self);

  fn to_response(&self) -> serde_json::Value
  where
    Self: Serialize,
  {
    serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
  }
}

pub trait WithValidation: Sized {
  fn validate_field<F>(
    &self,
    field_name: &str,
    value: &F,
    validator: impl Fn(&F) -> Option<String>,
  ) -> Option<ValidationError>;
}
