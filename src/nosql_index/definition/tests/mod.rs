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
