// Shared helpers for key-value-style providers (Redis, etcd, InfluxDB).
//
// These providers store each document under a key/prefix and filter/sort/
// paginate in memory after the read, so the helpers are identical.

use crate::query::Filter;
use crate::utils::{compare_values, generate_id};
use serde_json::Value;

pub fn document_id(doc: &Value) -> Option<String> {
  doc
    .get("id")
    .and_then(Value::as_str)
    .filter(|id| !id.is_empty())
    .map(str::to_string)
}

pub fn document_id_or_new(doc: &Value) -> String {
  document_id(doc).unwrap_or_else(generate_id)
}

pub fn select_documents(
  documents: Vec<Value>,
  filter: Option<&Filter>,
  skip: Option<u64>,
  limit: Option<u64>,
  sort_by: Option<&str>,
  sort_asc: bool,
) -> Vec<Value> {
  let mut results: Vec<Value> = documents
    .into_iter()
    .filter(|doc| filter.is_none_or(|f| f.matches(doc)))
    .collect();

  if let Some(field) = sort_by {
    results.sort_by(|a, b| {
      let ordering = compare_values(a.get(field), b.get(field));
      if sort_asc {
        ordering
      } else {
        ordering.reverse()
      }
    });
  }

  let skip = skip.unwrap_or(0) as usize;
  let take = limit.map_or(usize::MAX, |n| n as usize);
  results.into_iter().skip(skip).take(take).collect()
}

