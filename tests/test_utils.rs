use nosql_orm::utils::{generate_id, short_id};

#[test]
fn test_generate_id_not_empty() {
  let id = generate_id();
  assert!(!id.is_empty());
}

#[test]
fn test_generate_id_unique() {
  let id1 = generate_id();
  let id2 = generate_id();
  assert_ne!(id1, id2);
}

#[test]
fn test_generate_id_is_uuid_format() {
  let id = generate_id();
  let parts: Vec<&str> = id.split('-').collect();
  assert_eq!(parts.len(), 5);
}

#[test]
fn test_short_id_length() {
  let id = short_id();
  assert_eq!(id.len(), 8);
}

#[test]
fn test_short_id_unique() {
  let id1 = short_id();
  let id2 = short_id();
  assert_ne!(id1, id2);
}

#[test]
fn test_short_id_no_dashes() {
  let id = short_id();
  assert!(!id.contains('-'));
}

#[test]
fn test_short_id_is_hex() {
  let id = short_id();
  assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
}
