use crate::error::{OrmError, OrmResult};
use serde_json::Value;

pub trait ZeroCopyDeserialize<T> {
  fn zero_copy_deserialize(data: &[u8]) -> OrmResult<T>;
}

impl ZeroCopyDeserialize<String> for String {
  fn zero_copy_deserialize(data: &[u8]) -> OrmResult<String> {
    String::from_utf8(data.to_vec()).map_err(|e| OrmError::Internal(format!("utf8 error: {}", e)))
  }
}

pub struct DirectDeserializer;

impl DirectDeserializer {
  pub fn deserialize_string(value: &Value) -> Option<String> {
    match value {
      Value::String(s) => Some(s.clone()),
      _ => None,
    }
  }

  pub fn deserialize_i64(value: &Value) -> Option<i64> {
    match value {
      Value::Number(n) => n.as_i64(),
      _ => None,
    }
  }

  pub fn deserialize_u64(value: &Value) -> Option<u64> {
    match value {
      Value::Number(n) => n.as_u64(),
      _ => None,
    }
  }

  pub fn deserialize_f64(value: &Value) -> Option<f64> {
    match value {
      Value::Number(n) => n.as_f64(),
      _ => None,
    }
  }

  pub fn deserialize_bool(value: &Value) -> Option<bool> {
    match value {
      Value::Bool(b) => Some(*b),
      _ => None,
    }
  }

  pub fn deserialize_str(value: &Value) -> Option<&str> {
    match value {
      Value::String(s) => Some(s.as_str()),
      _ => None,
    }
  }
}

pub struct LazyValue<'a> {
  value: &'a Value,
}

impl<'a> LazyValue<'a> {
  pub fn new(value: &'a Value) -> Self {
    Self { value }
  }

  pub fn as_str(&self) -> Option<&str> {
    DirectDeserializer::deserialize_str(self.value)
  }

  pub fn as_i64(&self) -> Option<i64> {
    DirectDeserializer::deserialize_i64(self.value)
  }

  pub fn as_u64(&self) -> Option<u64> {
    DirectDeserializer::deserialize_u64(self.value)
  }

  pub fn as_f64(&self) -> Option<f64> {
    DirectDeserializer::deserialize_f64(self.value)
  }

  pub fn as_bool(&self) -> Option<bool> {
    DirectDeserializer::deserialize_bool(self.value)
  }

  pub fn is_null(&self) -> bool {
    self.value.is_null()
  }

  pub fn is_array(&self) -> bool {
    self.value.is_array()
  }

  pub fn is_object(&self) -> bool {
    self.value.is_object()
  }

  pub fn as_array(&self) -> Option<&Vec<Value>> {
    self.value.as_array()
  }

  pub fn as_object(&self) -> Option<&serde_json::Map<String, Value>> {
    self.value.as_object()
  }
}

pub struct TypedArrayDeserializer;

impl TypedArrayDeserializer {
  pub fn deserialize_strings(values: &[Value]) -> Vec<String> {
    values
      .iter()
      .filter_map(DirectDeserializer::deserialize_string)
      .collect()
  }

  pub fn deserialize_i64s(values: &[Value]) -> Vec<i64> {
    values
      .iter()
      .filter_map(DirectDeserializer::deserialize_i64)
      .collect()
  }

  pub fn deserialize_u64s(values: &[Value]) -> Vec<u64> {
    values
      .iter()
      .filter_map(DirectDeserializer::deserialize_u64)
      .collect()
  }

  pub fn deserialize_f64s(values: &[Value]) -> Vec<f64> {
    values
      .iter()
      .filter_map(DirectDeserializer::deserialize_f64)
      .collect()
  }
}

#[cfg(test)]
mod tests;
