use serde_json::Value;

pub struct Embedder;

impl Embedder {
  pub fn flatten(doc: &Value, embedded_fields: &[&str]) -> Value {
    let mut result = doc.clone();

    for field in embedded_fields {
      let embedded = result.get(*field).cloned();
      if let Some(embedded_obj) = embedded.and_then(|e| e.as_object().cloned()) {
        for (key, value) in embedded_obj {
          let prefixed_key = format!("{}_{}", field, key);
          result.as_object_mut().unwrap().insert(prefixed_key, value);
        }
        result.as_object_mut().unwrap().remove(*field);
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
          result.as_object_mut().unwrap().remove(&key);
        }
      }

      if !embedded.is_empty() {
        result
          .as_object_mut()
          .unwrap()
          .insert(field.to_string(), Value::Object(embedded));
      }
    }

    result
  }
}
