use super::{
  escape_field_string, escape_measurement, escape_tag, quote_literal, quote_measurement,
  validate_collection, value_kind,
};
use crate::error::OrmError;
use crate::providers::kv_common::{document_id, document_id_or_new, select_documents};
use crate::query::Filter;
use serde_json::json;

// The escaping helpers are the bug-prone part of this provider: a tag that is
// over-escaped on write can never be matched again on read, and the failure
// looks like "the document is missing" rather than "the query is wrong".
// They are pure, so they are asserted offline. Everything needing a server is
// #[ignore]d with the reason in the attribute.

#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_insert_requires_live_server() {
  // Reported through the integration suite, which skips when absent.
}

// The regression this suite exists for: `"` must NOT be escaped in a tag
// value. InfluxDB does not unescape it, so escaping stores a literal
// backslash and the later `WHERE id='a"b'` finds nothing. Spaces still get
// escaped — they are what terminates the tag set in line protocol.
#[test]
fn escape_tag_leaves_double_quotes_alone() {
  assert_eq!(escape_tag(r#"a"b"#), r#"a"b"#);
  assert_eq!(escape_tag(r#"it's a "trap""#), r#"it's\ a\ "trap""#);
}

#[test]
fn escape_tag_escapes_backslash_comma_equals_and_space() {
  assert_eq!(escape_tag(r"a\b"), r"a\\b");
  assert_eq!(escape_tag("a,b"), r"a\,b");
  assert_eq!(escape_tag("a=b"), r"a\=b");
  assert_eq!(escape_tag("a b"), r"a\ b");
  // Several at once, in order.
  assert_eq!(escape_tag(r"a, b=c"), r"a\,\ b\=c");
}

#[test]
fn escape_tag_leaves_apostrophes_alone() {
  // An apostrophe is legal raw in line protocol; only the InfluxQL literal
  // form needs it escaped, which `quote_literal` handles separately.
  assert_eq!(escape_tag("it's"), "it's");
}

#[test]
fn escape_measurement_escapes_backslash_comma_and_space() {
  assert_eq!(escape_measurement("tasks"), "tasks");
  assert_eq!(escape_measurement("my-meas.1"), "my-meas.1");
  assert_eq!(escape_measurement("a b"), r"a\ b");
  assert_eq!(escape_measurement("a,b"), r"a\,b");
  assert_eq!(escape_measurement(r"a\b"), r"a\\b");
  // `=` is legal unquoted in a measurement, so it is left alone.
  assert_eq!(escape_measurement("a=b"), "a=b");
}

#[test]
fn escape_field_string_quotes_and_escapes() {
  assert_eq!(escape_field_string("plain"), r#""plain""#);
  assert_eq!(escape_field_string(r#"say "hi""#), r#""say \"hi\"""#);
  assert_eq!(escape_field_string(r"a\b"), r#""a\\b""#);
  // A JSON document is the actual payload, so check it byte for byte.
  let doc = r#"{"name":"Ann","score":3}"#;
  assert_eq!(
    escape_field_string(doc),
    r#""{\"name\":\"Ann\",\"score\":3}""#
  );
}

#[test]
fn quote_measurement_wraps_in_double_quotes() {
  assert_eq!(quote_measurement("tasks"), r#""tasks""#);
  assert_eq!(quote_measurement(r#"we"ird"#), r#""we\"ird""#);
  assert_eq!(quote_measurement(r"a\b"), r#""a\\b""#);
}

#[test]
fn quote_literal_escapes_single_quotes_and_backslashes() {
  assert_eq!(quote_literal("plain"), "'plain'");
  assert_eq!(quote_literal("it's"), r"'it\'s'");
  assert_eq!(quote_literal(r"a\b"), r"'a\\b'");
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
  assert_eq!(document_id(&json!({"id": 42})), None);
}

#[test]
fn document_id_or_new_generates_one_when_absent() {
  assert_eq!(document_id_or_new(&json!({"id": "keep"})), "keep");
  let generated = document_id_or_new(&json!({"name": "a"}));
  assert!(!generated.is_empty());
  assert_ne!(
    document_id_or_new(&json!({"name": "a"})),
    document_id_or_new(&json!({"name": "a"}))
  );
}

#[test]
fn validate_collection_rejects_only_empty_names() {
  assert!(validate_collection("").is_err());
  // Everything else is safe: every use is quoted or escaped.
  assert!(validate_collection("tasks").is_ok());
  assert!(validate_collection("my-meas.1").is_ok());
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

  let ascending = select_documents(docs.clone(), None, None, None, Some("score"), true);
  assert_eq!(ids(&ascending), vec!["c-2", "c-3", "c-1"]);

  let descending = select_documents(docs.clone(), None, None, None, Some("score"), false);
  assert_eq!(ids(&descending), vec!["c-1", "c-3", "c-2"]);

  let page = select_documents(docs.clone(), None, Some(1), Some(1), Some("score"), true);
  assert_eq!(ids(&page), vec!["c-3"]);

  let filtered = select_documents(
    docs,
    Some(&Filter::Eq("name".to_string(), json!("Ann"))),
    None,
    None,
    None,
    true,
  );
  assert_eq!(ids(&filtered), vec!["c-1"]);
}

#[test]
fn select_documents_with_no_results_is_empty() {
  let empty: Vec<serde_json::Value> = Vec::new();
  assert!(select_documents(empty, None, None, None, None, true).is_empty());
}

fn ids(docs: &[serde_json::Value]) -> Vec<String> {
  docs
    .iter()
    .map(|d| d["id"].as_str().map(str::to_string).unwrap_or_default())
    .collect()
}
