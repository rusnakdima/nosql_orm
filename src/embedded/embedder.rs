use crate::query::Filter;
use serde_json::Value;

pub struct Embedder;

impl Embedder {
  pub fn flatten(doc: &Value, embedded_fields: &[&str]) -> Value {
    let mut result = doc.clone();

    for field in embedded_fields {
      let embedded = result.get(*field).cloned();
      if let Some(embedded_obj) = embedded.and_then(|e| e.as_object().cloned()) {
        // `result.get` returned `Some`, so `result` is an object. Bind it
        // once instead of re-asserting that with an unwrap.
        if let Value::Object(obj) = &mut result {
          for (key, value) in embedded_obj {
            let prefixed_key = format!("{}_{}", field, key);
            obj.insert(prefixed_key, value);
          }
          obj.remove(*field);
        }
      }
    }

    result
  }

  pub fn unflatten(doc: &Value, embedded_fields: &[&str]) -> Value {
    let mut result = doc.clone();

    for field in embedded_fields {
      let prefix = format!("{}_", field);
      let mut embedded = serde_json::Map::new();

      let keys_to_remove: Vec<String> = result
        .as_object()
        .map(|obj| {
          obj
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect()
        })
        .unwrap_or_default();

      for key in keys_to_remove {
        if let Some(stripped) = key.strip_prefix(&prefix) {
          if let Some(value) = result.get(&key) {
            embedded.insert(stripped.to_string(), value.clone());
          }
          // `keys_to_remove` came from `result.as_object()`, so `result` is
          // an object here.
          if let Value::Object(obj) = &mut result {
            obj.remove(&key);
          }
        }
      }

      if !embedded.is_empty() {
        if let Value::Object(obj) = &mut result {
          obj.insert(field.to_string(), Value::Object(embedded));
        }
      }
    }

    result
  }

  pub fn rewrite_filter(filter: &Filter, embedded_fields: &[&str]) -> Filter {
    if embedded_fields.is_empty() {
      return filter.clone();
    }
    match filter {
      Filter::Eq(field, value) => {
        Filter::Eq(map_embedded_path(field, embedded_fields), value.clone())
      }
      Filter::Ne(field, value) => {
        Filter::Ne(map_embedded_path(field, embedded_fields), value.clone())
      }
      Filter::Gt(field, value) => {
        Filter::Gt(map_embedded_path(field, embedded_fields), value.clone())
      }
      Filter::Gte(field, value) => {
        Filter::Gte(map_embedded_path(field, embedded_fields), value.clone())
      }
      Filter::Lt(field, value) => {
        Filter::Lt(map_embedded_path(field, embedded_fields), value.clone())
      }
      Filter::Lte(field, value) => {
        Filter::Lte(map_embedded_path(field, embedded_fields), value.clone())
      }
      Filter::In(field, values) => {
        Filter::In(map_embedded_path(field, embedded_fields), values.clone())
      }
      Filter::NotIn(field, values) => {
        Filter::NotIn(map_embedded_path(field, embedded_fields), values.clone())
      }
      Filter::ArrayContains(field, value) => {
        Filter::ArrayContains(map_embedded_path(field, embedded_fields), value.clone())
      }
      Filter::Contains(field, sub) => {
        Filter::Contains(map_embedded_path(field, embedded_fields), sub.clone())
      }
      Filter::StartsWith(field, prefix) => {
        Filter::StartsWith(map_embedded_path(field, embedded_fields), prefix.clone())
      }
      Filter::EndsWith(field, suffix) => {
        Filter::EndsWith(map_embedded_path(field, embedded_fields), suffix.clone())
      }
      Filter::Like(field, pattern) => {
        Filter::Like(map_embedded_path(field, embedded_fields), pattern.clone())
      }
      Filter::IsNull(field) => Filter::IsNull(map_embedded_path(field, embedded_fields)),
      Filter::IsNotNull(field) => Filter::IsNotNull(map_embedded_path(field, embedded_fields)),
      Filter::Between(field, min, max) => Filter::Between(
        map_embedded_path(field, embedded_fields),
        min.clone(),
        max.clone(),
      ),
      Filter::And(filters) => Filter::And(
        filters
          .iter()
          .map(|f| Self::rewrite_filter(f, embedded_fields))
          .collect(),
      ),
      Filter::Or(filters) => Filter::Or(
        filters
          .iter()
          .map(|f| Self::rewrite_filter(f, embedded_fields))
          .collect(),
      ),
      Filter::Not(inner) => Filter::Not(Box::new(Self::rewrite_filter(inner, embedded_fields))),
    }
  }
}

fn map_embedded_path(field: &str, embedded_fields: &[&str]) -> String {
  let mut parts = field.splitn(2, '.');
  let head = match parts.next() {
    Some(head) => head,
    None => return field.to_string(),
  };
  if !embedded_fields.contains(&head) {
    return field.to_string();
  }
  match parts.next() {
    Some(tail) => format!("{}_{}", head, tail),
    None => field.to_string(),
  }
}
