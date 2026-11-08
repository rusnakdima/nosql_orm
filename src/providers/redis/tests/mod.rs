use super::{document_id, document_id_or_new, select_documents};
use crate::error::{OrmError, OrmResult};
use crate::query::Filter;
use serde_json::json;

#[ignore]
#[tokio::test]
async fn redis_insert_requires_live_server() -> OrmResult<()> {
  // Needs a live Redis server, so there is nothing to assert offline.
  // Report the missing capability as an error instead of panicking.
  Err(OrmError::NotSupported(
    "redis insert needs a live Redis server".to_string(),
  ))
}

#[ignore]
#[tokio::test]
async fn redis_find_by_id_requires_live_server() -> OrmResult<()> {
  // Needs a live Redis server, so there is nothing to assert offline.
  // Report the missing capability as an error instead of panicking.
  Err(OrmError::NotSupported(
    "redis find_by_id needs a live Redis server".to_string(),
  ))
}

#[test]
fn document_id_reads_a_caller_supplied_id() {
  assert_eq!(
    document_id(&json!({"id": "caller-id", "name": "a"})),
    Some("caller-id".to_string())
  );
}

#[test]
fn document_id_ignores_a_missing_or_empty_id() {
  assert_eq!(document_id(&json!({"name": "a"})), None);
  assert_eq!(document_id(&json!({"id": "", "name": "a"})), None);
  assert_eq!(document_id(&json!({"id": 7})), None);
}

#[test]
fn document_id_or_new_preserves_the_caller_id() {
  assert_eq!(document_id_or_new(&json!({"id": "caller-id"})), "caller-id");
}

#[test]
fn document_id_or_new_generates_an_id_when_absent() {
  let first = document_id_or_new(&json!({"name": "a"}));
  let second = document_id_or_new(&json!({"name": "a"}));
  assert!(!first.is_empty());
  assert!(!second.is_empty());
  assert_ne!(first, second);
}

#[test]
fn empty_id_is_replaced_with_a_generated_one() {
  let id = document_id_or_new(&json!({"id": ""}));
  assert!(!id.is_empty());
}

fn docs() -> Vec<serde_json::Value> {
  vec![
    json!({"id": "a", "name": "Ann", "rank": 3}),
    json!({"id": "b", "name": "Bob", "rank": 1}),
    json!({"id": "c", "name": "Cid", "rank": 2}),
    json!({"id": "d", "name": "Dot", "rank": 4}),
  ]
}

#[test]
fn filtering_happens_before_skip_and_limit() {
  let filter = Filter::Gt("rank".to_string(), json!(1));
  let page = select_documents(docs(), Some(&filter), Some(1), Some(2), None, true);
  // Every fixture document carries a string id, so a None here would mean
  // select_documents returned a document the fixture never wrote. Comparing
  // the Options states that instead of panicking on it.
  let ids: Vec<Option<&str>> = page.iter().map(|doc| doc["id"].as_str()).collect();
  assert_eq!(ids, vec![Some("c"), Some("d")]);
}

#[test]
fn sort_ascending_uses_the_named_field() {
  let sorted = select_documents(docs(), None, None, None, Some("name"), true);
  let names: Vec<Option<&str>> = sorted.iter().map(|doc| doc["name"].as_str()).collect();
  assert_eq!(names, vec![Some("Ann"), Some("Bob"), Some("Cid"), Some("Dot")]);
}

#[test]
fn sort_descending_reverses_the_order() {
  let sorted = select_documents(docs(), None, None, None, Some("rank"), false);
  let ranks: Vec<Option<i64>> = sorted.iter().map(|doc| doc["rank"].as_i64()).collect();
  assert_eq!(ranks, vec![Some(4), Some(3), Some(2), Some(1)]);
}

#[test]
fn sort_then_page_returns_the_requested_window() {
  let page = select_documents(docs(), None, Some(1), Some(2), Some("rank"), true);
  let ranks: Vec<Option<i64>> = page.iter().map(|doc| doc["rank"].as_i64()).collect();
  assert_eq!(ranks, vec![Some(2), Some(3)]);
}

#[test]
fn limit_zero_returns_nothing() {
  let page = select_documents(docs(), None, None, Some(0), None, true);
  assert!(page.is_empty());
}
