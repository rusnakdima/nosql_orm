use crate::query::Filter;
use crate::utils::DocumentExt;
use serde_json::Value;

pub fn filter_not_deleted(docs: Vec<Value>) -> Vec<Value> {
  docs
    .into_iter()
    .filter(|d| match d.get("deleted_at") {
      Some(v) if v.is_null() => true,
      Some(v) if v.as_str().is_some_and(|s| s.is_empty()) => true,
      Some(_) => false,
      None => true,
    })
    .collect()
}

pub fn apply_filter(filter: Option<&Filter>) -> Option<Filter> {
  if let Some(f) = filter {
    Some(Filter::And(vec![
      f.clone(),
      Filter::Or(vec![
        Filter::IsNull("deleted_at".to_string()),
        Filter::Eq("deleted_at".to_string(), Value::String("".to_string())),
      ]),
    ]))
  } else {
    Some(Filter::Or(vec![
      Filter::IsNull("deleted_at".to_string()),
      Filter::Eq("deleted_at".to_string(), Value::String("".to_string())),
    ]))
  }
}

pub fn inject_collection(docs: Vec<Value>, collection: &str) -> Vec<Value> {
  docs
    .into_iter()
    .map(|doc| doc.inject_collection(collection))
    .collect()
}
