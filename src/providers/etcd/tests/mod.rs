use super::{validate_collection, value_kind};
use crate::error::{OrmError, OrmResult};
use crate::providers::kv_common::{document_id, document_id_or_new, select_documents};
use crate::query::Filter;
use serde_json::json;

// The helpers are pure, so they are asserted offline. Everything that needs
// a server is #[ignore]d with the reason in the attribute, matching the
// convention the Redis suite uses.

#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_insert_requires_live_server() -> OrmResult<()> {
  Err(OrmError::NotSupported(
    "etcd insert needs a running etcd server".to_string(),
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
fn document_id_ignores_missing_and_empty_ids() {
  assert_eq!(document_id(&json!({"name": "a"})), None);
  assert_eq!(document_id(&json!({"id": ""})), None);
  // A non-string id is not an id.
  assert_eq!(document_id(&json!({"id": 42})), None);
}

#[test]
fn document_id_or_new_generates_one_when_absent() {
  assert_eq!(
    document_id_or_new(&json!({"id": "keep"})),
    "keep".to_string()
  );
  let generated = document_id_or_new(&json!({"name": "a"}));
  assert!(!generated.is_empty(), "generated id must not be empty");
  // Two calls must not collide.
  assert_ne!(
    document_id_or_new(&json!({"name": "a"})),
    document_id_or_new(&json!({"name": "a"}))
  );
}

// A collection name becomes part of the key prefix, so '/' would make the
// prefix ambiguous — it must be rejected.
#[test]
fn validate_collection_rejects_empty_and_slash() {
  assert!(validate_collection("").is_err());
  assert!(validate_collection("a/b").is_err());
  assert!(validate_collection("tasks").is_ok());
  assert!(validate_collection("with space").is_ok());
}

#[test]
fn validate_collection_error_is_invalid_input() {
  match validate_collection("") {
    Err(OrmError::InvalidInput(_)) => {}
    other => panic!("expected InvalidInput, got {:?}", other),
  }
}

#[test]
fn value_kind_names_every_json_type() {
  assert_eq!(value_kind(&json!(null)), "null");
  assert_eq!(value_kind(&json!(true)), "boolean");
  assert_eq!(value_kind(&json!(1)), "number");
  assert_eq!(value_kind(&json!("s")), "string");
  assert_eq!(value_kind(&json!([1])), "array");
  assert_eq!(value_kind(&json!({"a": 1})), "object");
}

#[test]
fn select_documents_filters_sorts_and_paginates() {
  let docs = vec![
    json!({"id": "c-1", "name": "Ann", "score": 3}),
    json!({"id": "c-2", "name": "Bob", "score": 1}),
    json!({"id": "c-3", "name": "Cid", "score": 2}),
  ];

  // Unfiltered, ascending by score.
  let ascending = select_documents(docs.clone(), None, None, None, Some("score"), true);
  assert_eq!(
    ascending
      .iter()
      .map(|d| d["id"].as_str().unwrap())
      .collect::<Vec<_>>(),
    vec!["c-2", "c-3", "c-1"]
  );

  // Descending.
  let descending = select_documents(docs.clone(), None, None, None, Some("score"), false);
  assert_eq!(
    descending
      .iter()
      .map(|d| d["id"].as_str().unwrap())
      .collect::<Vec<_>>(),
    vec!["c-1", "c-3", "c-2"]
  );

  // Skip one, take one.
  let page = select_documents(docs.clone(), None, Some(1), Some(1), Some("score"), true);
  assert_eq!(page.len(), 1);
  assert_eq!(page[0]["id"], "c-3");

  // Filter selects exactly the match.
  let filtered = select_documents(
    docs,
    Some(&Filter::Eq("name".to_string(), json!("Ann"))),
    None,
    None,
    None,
    true,
  );
  assert_eq!(filtered.len(), 1);
  assert_eq!(filtered[0]["id"], "c-1");
}

#[test]
fn select_documents_with_no_results_is_empty() {
  let empty: Vec<serde_json::Value> = Vec::new();
  assert!(select_documents(empty, None, None, None, None, true).is_empty());
}
