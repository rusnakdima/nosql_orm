//! NoSQL index definition types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use crate::query::Filter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NosqlIndexType {
  SingleField,
  Compound,
  Text,
  Geospatial2dsphere,
  Geospatial2d,
  Hashed,
  Ttl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NosqlIndexInfo {
  pub name: String,
  pub namespace: String,
  pub unique: bool,
  pub sparse: bool,
  pub ttl_seconds: Option<u32>,
  pub version: Option<String>,
  pub index_type: String,
  pub fields: Vec<(String, i32)>,
}

impl Default for NosqlIndexInfo {
  fn default() -> Self {
    Self {
      name: String::new(),
      namespace: String::new(),
      unique: false,
      sparse: false,
      ttl_seconds: None,
      version: None,
      index_type: "single".to_string(),
      fields: Vec::new(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct NosqlIndex {
  name: Option<String>,
  fields: Vec<(String, i32)>,
  index_type: NosqlIndexType,
  unique: bool,
  sparse: bool,
  ttl_seconds: Option<u32>,
  partial_filter: Option<Filter>,
  weights: Option<HashMap<String, i32>>,
  default_language: Option<String>,
  sphere_version: Option<i32>,
  two_d_precision: Option<f64>,
}

impl NosqlIndex {
  pub fn single(field: &str, order: i32) -> Self {
    Self {
      name: None,
      fields: vec![(field.to_string(), order)],
      index_type: NosqlIndexType::SingleField,
      unique: false,
      sparse: false,
      ttl_seconds: None,
      partial_filter: None,
      weights: None,
      default_language: None,
      sphere_version: None,
      two_d_precision: None,
    }
  }

  pub fn compound(fields: &[(&str, i32)]) -> Self {
    Self {
      name: None,
      fields: fields.iter().map(|(f, o)| (f.to_string(), *o)).collect(),
      index_type: NosqlIndexType::Compound,
      unique: false,
      sparse: false,
      ttl_seconds: None,
      partial_filter: None,
      weights: None,
      default_language: None,
      sphere_version: None,
      two_d_precision: None,
    }
  }

  pub fn text(fields: &[(&str, i32)]) -> Self {
    Self {
      name: None,
      fields: fields.iter().map(|(f, w)| (f.to_string(), *w)).collect(),
      index_type: NosqlIndexType::Text,
      unique: false,
      sparse: false,
      ttl_seconds: None,
      partial_filter: None,
      weights: Some(fields.iter().map(|(f, w)| (f.to_string(), *w)).collect()),
      default_language: Some("english".to_string()),
      sphere_version: None,
      two_d_precision: None,
    }
  }

  pub fn geospatial_2dsphere(field: &str) -> Self {
    Self {
      name: None,
      fields: vec![(field.to_string(), 1)],
      index_type: NosqlIndexType::Geospatial2dsphere,
      unique: false,
      sparse: false,
      ttl_seconds: None,
      partial_filter: None,
      weights: None,
      default_language: None,
      sphere_version: Some(3),
      two_d_precision: None,
    }
  }

  pub fn geospatial_2d(field: &str) -> Self {
    Self {
      name: None,
      fields: vec![(field.to_string(), 1)],
      index_type: NosqlIndexType::Geospatial2d,
      unique: false,
      sparse: false,
      ttl_seconds: None,
      partial_filter: None,
      weights: None,
      default_language: None,
      sphere_version: None,
      two_d_precision: Some(25.0),
    }
  }

  pub fn hashed(field: &str) -> Self {
    Self {
      name: None,
      fields: vec![(field.to_string(), 1)],
      index_type: NosqlIndexType::Hashed,
      unique: false,
      sparse: false,
      ttl_seconds: None,
      partial_filter: None,
      weights: None,
      default_language: None,
      sphere_version: None,
      two_d_precision: None,
    }
  }

  pub fn ttl(field: &str, expire_after_seconds: u32) -> Self {
    Self {
      name: None,
      fields: vec![(field.to_string(), 1)],
      index_type: NosqlIndexType::Ttl,
      unique: false,
      sparse: false,
      ttl_seconds: Some(expire_after_seconds),
      partial_filter: None,
      weights: None,
      default_language: None,
      sphere_version: None,
      two_d_precision: None,
    }
  }

  pub fn name(mut self, name: &str) -> Self {
    self.name = Some(name.to_string());
    self
  }

  pub fn unique(mut self) -> Self {
    self.unique = true;
    self
  }

  pub fn sparse(mut self) -> Self {
    self.sparse = true;
    self
  }

  pub fn partial_filter(mut self, filter: Filter) -> Self {
    self.partial_filter = Some(filter);
    self
  }

  pub fn default_language(mut self, lang: &str) -> Self {
    self.default_language = Some(lang.to_string());
    self
  }

  pub fn sphere_version(mut self, version: i32) -> Self {
    self.sphere_version = Some(version);
    self
  }

  pub fn two_d_precision(mut self, precision: f64) -> Self {
    self.two_d_precision = Some(precision);
    self
  }

  pub fn get_name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  pub fn get_fields(&self) -> &[(String, i32)] {
    &self.fields
  }

  pub fn get_index_type(&self) -> NosqlIndexType {
    self.index_type
  }

  pub fn is_unique(&self) -> bool {
    self.unique
  }

  pub fn is_sparse(&self) -> bool {
    self.sparse
  }

  pub fn get_ttl_seconds(&self) -> Option<u32> {
    self.ttl_seconds
  }

  pub fn get_partial_filter(&self) -> Option<&Filter> {
    self.partial_filter.as_ref()
  }

  pub fn get_weights(&self) -> Option<&HashMap<String, i32>> {
    self.weights.as_ref()
  }

  pub fn get_default_language(&self) -> Option<&str> {
    self.default_language.as_deref()
  }

  pub fn get_sphere_version(&self) -> Option<i32> {
    self.sphere_version
  }

  pub fn get_two_d_precision(&self) -> Option<f64> {
    self.two_d_precision
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_single_index() {
    let idx = NosqlIndex::single("email", 1).unique();
    assert_eq!(idx.index_type, NosqlIndexType::SingleField);
    assert!(idx.unique);
    assert_eq!(idx.fields, vec![("email".to_string(), 1)]);
  }

  #[test]
  fn test_compound_index() {
    let idx = NosqlIndex::compound(&[("a", 1), ("b", -1)]);
    assert_eq!(idx.index_type, NosqlIndexType::Compound);
    assert_eq!(idx.fields.len(), 2);
  }

  #[test]
  fn test_text_index() {
    let idx = NosqlIndex::text(&[("title", 10), ("body", 1)]);
    assert_eq!(idx.index_type, NosqlIndexType::Text);
    assert!(idx.weights.is_some());
  }

  #[test]
  fn test_ttl_index() {
    let idx = NosqlIndex::ttl("created_at", 86400);
    assert_eq!(idx.index_type, NosqlIndexType::Ttl);
    assert_eq!(idx.ttl_seconds, Some(86400));
  }

  #[test]
  fn test_builder_chain() {
    let idx = NosqlIndex::single("email", 1)
      .name("idx_email")
      .unique()
      .sparse();

    assert_eq!(idx.name, Some("idx_email".to_string()));
    assert!(idx.unique);
    assert!(idx.sparse);
  }
}
