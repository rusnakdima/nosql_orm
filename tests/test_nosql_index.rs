use nosql_orm::nosql_index::{NosqlIndex, NosqlIndexInfo, NosqlIndexType};

#[test]
fn test_nosql_index_single_field() {
  let idx = NosqlIndex::single("email", 1);
  assert_eq!(idx.get_index_type(), NosqlIndexType::SingleField);
  assert_eq!(idx.get_fields(), &[("email".to_string(), 1)]);
  assert!(!idx.is_unique());
  assert!(!idx.is_sparse());
}

#[test]
fn test_nosql_index_compound() {
  let idx = NosqlIndex::compound(&[("user_id", 1), ("created_at", -1)]);
  assert_eq!(idx.get_index_type(), NosqlIndexType::Compound);
  assert_eq!(idx.get_fields().len(), 2);
}

#[test]
fn test_nosql_index_text() {
  let idx = NosqlIndex::text(&[("title", 10), ("body", 1)]);
  assert_eq!(idx.get_index_type(), NosqlIndexType::Text);
  assert!(idx.get_weights().is_some());
  let weights = idx.get_weights().unwrap();
  assert_eq!(weights.get("title"), Some(&10));
  assert_eq!(weights.get("body"), Some(&1));
}

#[test]
fn test_nosql_index_ttl() {
  let idx = NosqlIndex::ttl("created_at", 86400);
  assert_eq!(idx.get_index_type(), NosqlIndexType::Ttl);
  assert_eq!(idx.get_ttl_seconds(), Some(86400));
}

#[test]
fn test_nosql_index_geospatial_2dsphere() {
  let idx = NosqlIndex::geospatial_2dsphere("location");
  assert_eq!(idx.get_index_type(), NosqlIndexType::Geospatial2dsphere);
  assert_eq!(idx.get_sphere_version(), Some(3));
}

#[test]
fn test_nosql_index_geospatial_2d() {
  let idx = NosqlIndex::geospatial_2d("coordinates").two_d_precision(30.0);
  assert_eq!(idx.get_index_type(), NosqlIndexType::Geospatial2d);
  assert_eq!(idx.get_two_d_precision(), Some(30.0));
}

#[test]
fn test_nosql_index_hashed() {
  let idx = NosqlIndex::hashed("user_id");
  assert_eq!(idx.get_index_type(), NosqlIndexType::Hashed);
}

#[test]
fn test_nosql_index_builder_chain() {
  let idx = NosqlIndex::single("email", 1)
    .name("idx_email")
    .unique()
    .sparse()
    .partial_filter(nosql_orm::query::Filter::Eq(
      "active".to_string(),
      serde_json::json!(true),
    ));

  assert_eq!(idx.get_name(), Some("idx_email"));
  assert!(idx.is_unique());
  assert!(idx.is_sparse());
  assert!(idx.get_partial_filter().is_some());
}

#[test]
fn test_nosql_index_default_language() {
  let idx = NosqlIndex::text(&[("title", 1)]).default_language("spanish");
  assert_eq!(idx.get_default_language(), Some("spanish"));
}

#[test]
fn test_nosql_index_info_default() {
  let info = NosqlIndexInfo::default();
  assert_eq!(info.name, "");
  assert!(!info.unique);
  assert!(!info.sparse);
  assert!(info.ttl_seconds.is_none());
}
