//! NoSQL-specific index types and management.
//!
//! This module provides index abstractions tailored for NoSQL databases,
//! particularly MongoDB, with support for various index types.

pub mod manager;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use manager::IndexManager;

/// Index types specific to NoSQL/MongoDB databases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NosqlIndexType {
  /// Single field ascending/descending index.
  SingleField,
  /// Compound index on multiple fields.
  Compound,
  /// Text index for full-text search.
  Text,
  /// Geospatial index on a sphere (for GeoJSON points).
  Geospatial2dsphere,
  /// Legacy geospatial index on a flat plane.
  Geospatial2d,
  /// Hashed index for sharding.
  Hashed,
  /// TTL index for automatic document expiration.
  Ttl,
}

/// Information about an existing index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NosqlIndexInfo {
  /// Index name.
  pub name: String,
  /// Index namespace (collection).
  pub namespace: String,
  /// Whether the index is unique.
  pub unique: bool,
  /// Whether the index is sparse.
  pub sparse: bool,
  /// TTL in seconds if applicable.
  pub ttl_seconds: Option<u32>,
  /// Index version as string (can be numeric or special values like "1", "2", "3").
  pub version: Option<String>,
  /// Index type.
  pub index_type: String,
  /// Fields in the index with their sort order.
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

/// A NoSQL index definition.
///
/// Use the builder methods to create indexes with specific configurations.
///
/// # Examples
///
/// ```rust
/// use nosql_orm::nosql_index::{NosqlIndex, NosqlIndexType};
///
/// // Single field unique index
/// let index = NosqlIndex::single("email", 1).unique(true).name("idx_email");
///
/// // Compound index
/// let index = NosqlIndex::compound(&[("user_id", 1), ("created_at", -1)]);
///
/// // Text index for full-text search
/// let index = NosqlIndex::text(&[("title", 10), ("description", 5)]);
///
/// // TTL index (30 days)
/// let index = NosqlIndex::ttl("created_at", 30 * 24 * 60 * 60);
/// ```
#[derive(Debug, Clone)]
pub struct NosqlIndex {
  /// Optional custom name for the index.
  name: Option<String>,
  /// Fields to index with their sort order (1 = asc, -1 = desc).
  fields: Vec<(String, i32)>,
  /// Type of index.
  index_type: NosqlIndexType,
  /// Whether to enforce uniqueness.
  unique: bool,
  /// Whether to index only non-null values.
  sparse: bool,
  /// TTL in seconds (for TTL indexes).
  ttl_seconds: Option<u32>,
  /// Partial filter expression (for partial indexes).
  partial_filter: Option<crate::query::Filter>,
  /// Field weights for text indexes.
  weights: Option<HashMap<String, i32>>,
  /// Default language for text indexes.
  default_language: Option<String>,
  /// 2dsphere index version (default 3).
  sphere_version: Option<i32>,
  /// 2d index precision (for legacy 2d indexes).
  two_d_precision: Option<f64>,
}

impl NosqlIndex {
  /// Create a single field index.
  ///
  /// # Arguments
  /// * `field` - The field name to index.
  /// * `order` - Sort order: 1 for ascending, -1 for descending.
  ///
  /// # Example
  ///
  /// ```rust
  /// let index = NosqlIndex::single("email", 1).unique(true);
  /// ```
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

  /// Create a compound index on multiple fields.
  ///
  /// # Arguments
  /// * `fields` - Slice of (field_name, sort_order) tuples.
  ///
  /// # Example
  ///
  /// ```rust
  /// let index = NosqlIndex::compound(&[
  ///     ("user_id", 1),
  ///     ("created_at", -1),
  /// ]);
  /// ```
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

  /// Create a text index for full-text search.
  ///
  /// # Arguments
  /// * `fields` - Slice of (field_name, weight) tuples. Higher weights
  ///   mean higher relevance scores in text search.
  ///
  /// # Example
  ///
  /// ```rust
  /// let index = NosqlIndex::text(&[
  ///     ("title", 10),
  ///     ("description", 5),
  ///     ("content", 1),
  /// ]).default_language("en");
  /// ```
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

  /// Create a 2dsphere geospatial index for GeoJSON points.
  ///
  /// # Example
  ///
  /// ```rust
  /// let index = NosqlIndex::geospatial_2dsphere("location");
  /// ```
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

  /// Create a legacy 2d geospatial index (flat plane).
  ///
  /// # Example
  ///
  /// ```rust
  /// let index = NosqlIndex::geospatial_2d("coordinates").two_d_precision(25.0);
  /// ```
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

  /// Create a hashed index for sharding.
  ///
  /// # Example
  ///
  /// ```rust
  /// let index = NosqlIndex::hashed("user_id");
  /// ```
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

  /// Create a TTL index for automatic document expiration.
  ///
  /// The field should be a Date or timestamp field. Documents will be
  /// automatically deleted after `expire_after_seconds` from the
  /// field value.
  ///
  /// # Arguments
  /// * `field` - The date field to use for TTL calculation.
  /// * `expire_after_seconds` - Seconds after the field date to delete documents.
  ///
  /// # Example
  ///
  /// ```rust
  /// // Delete documents 30 days after created_at
  /// let index = NosqlIndex::ttl("created_at", 30 * 24 * 60 * 60);
  /// ```
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

  /// Set a custom name for the index.
  ///
  /// If not set, MongoDB will generate a name based on the fields.
  pub fn name(mut self, name: &str) -> Self {
    self.name = Some(name.to_string());
    self
  }

  /// Make the index unique.
  ///
  /// No two documents can have the same indexed value(s).
  pub fn unique(mut self) -> Self {
    self.unique = true;
    self
  }

  /// Make the index sparse.
  ///
  /// Only index documents where the field exists and is not null.
  pub fn sparse(mut self) -> Self {
    self.sparse = true;
    self
  }

  /// Set partial filter expression for partial indexes.
  ///
  /// Only documents matching the filter will be indexed.
  pub fn partial_filter(mut self, filter: crate::query::Filter) -> Self {
    self.partial_filter = Some(filter);
    self
  }

  /// Set default language for text indexes.
  ///
  /// Default is "english".
  pub fn default_language(mut self, lang: &str) -> Self {
    self.default_language = Some(lang.to_string());
    self
  }

  /// Set 2dsphere index version (default 3).
  pub fn sphere_version(mut self, version: i32) -> Self {
    self.sphere_version = Some(version);
    self
  }

  /// Set precision for 2d indexes (default 25.0).
  pub fn two_d_precision(mut self, precision: f64) -> Self {
    self.two_d_precision = Some(precision);
    self
  }

  /// Get the index name.
  pub fn get_name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  /// Get the fields.
  pub fn get_fields(&self) -> &[(String, i32)] {
    &self.fields
  }

  /// Get the index type.
  pub fn get_index_type(&self) -> NosqlIndexType {
    self.index_type
  }

  /// Check if unique.
  pub fn is_unique(&self) -> bool {
    self.unique
  }

  /// Check if sparse.
  pub fn is_sparse(&self) -> bool {
    self.sparse
  }

  /// Get TTL seconds.
  pub fn get_ttl_seconds(&self) -> Option<u32> {
    self.ttl_seconds
  }

  /// Get partial filter.
  pub fn get_partial_filter(&self) -> Option<&crate::query::Filter> {
    self.partial_filter.as_ref()
  }

  /// Get text weights.
  pub fn get_weights(&self) -> Option<&HashMap<String, i32>> {
    self.weights.as_ref()
  }

  /// Get default language.
  pub fn get_default_language(&self) -> Option<&str> {
    self.default_language.as_deref()
  }

  /// Get sphere version.
  pub fn get_sphere_version(&self) -> Option<i32> {
    self.sphere_version
  }

  /// Get 2d precision.
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
