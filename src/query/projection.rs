use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Projection {
  pub select: Option<Vec<String>>,
  pub exclude: Option<Vec<String>>,
}

impl Projection {
  pub fn new() -> Self {
    Self {
      select: None,
      exclude: None,
    }
  }

  pub fn select(fields: &[&str]) -> Self {
    Self {
      select: Some(fields.iter().map(|s| s.to_string()).collect()),
      exclude: None,
    }
  }

  pub fn exclude(fields: &[&str]) -> Self {
    Self {
      select: None,
      exclude: Some(fields.iter().map(|s| s.to_string()).collect()),
    }
  }

  pub fn exclude_vec(fields: Vec<String>) -> Self {
    Self {
      select: None,
      exclude: Some(fields),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.select.is_none() && self.exclude.is_none()
  }

  pub fn apply(&self, doc: &Value) -> Value {
    if self.is_empty() {
      return doc.clone();
    }

    let obj = match doc.as_object() {
      Some(o) => o.clone(),
      None => return doc.clone(),
    };

    if let Some(ref select_fields) = self.select {
      let filtered: serde_json::Map<String, Value> = obj
        .into_iter()
        .filter(|(k, _)| select_fields.contains(k))
        .collect();
      return Value::Object(filtered);
    }

    if let Some(ref exclude_fields) = self.exclude {
      let filtered: serde_json::Map<String, Value> = obj
        .into_iter()
        .filter(|(k, _)| !exclude_fields.contains(k))
        .collect();
      return Value::Object(filtered);
    }

    doc.clone()
  }

  pub fn apply_recursive<'a>(&self, doc: &'a Value) -> Value
  where
    'a: 'a,
  {
    let mut filtered = self.apply(doc);

    if let Some(obj) = filtered.as_object_mut() {
      for (_key, val) in obj.iter_mut() {
        *val = self.apply_recursive(val);
      }
    } else if let Some(arr) = filtered.as_array_mut() {
      for item in arr.iter_mut() {
        *item = self.apply_recursive(item);
      }
    }

    filtered
  }
}

impl Default for Projection {
  fn default() -> Self {
    Self::new()
  }
}
