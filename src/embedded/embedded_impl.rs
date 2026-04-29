pub trait Embedded: Send + Sync + 'static {
  fn embedded_meta() -> EmbeddedMeta;
}

#[derive(Debug, Clone)]
pub struct EmbeddedMeta {
  pub prefix: Option<String>,
  pub flatten: bool,
}

impl EmbeddedMeta {
  pub fn new() -> Self {
    Self {
      prefix: None,
      flatten: false,
    }
  }

  pub fn prefix(mut self, prefix: &str) -> Self {
    self.prefix = Some(prefix.to_string());
    self
  }

  pub fn flatten(mut self) -> Self {
    self.flatten = true;
    self
  }
}

impl Default for EmbeddedMeta {
  fn default() -> Self {
    Self::new()
  }
}

pub trait EmbedExt {
  fn embed(&self, field: &str) -> serde_json::Value;
  fn unembed(&self, field: &str) -> serde_json::Value;
}

impl EmbedExt for serde_json::Value {
  fn embed(&self, field: &str) -> serde_json::Value {
    let mut result = serde_json::Map::new();

    if let Some(obj) = self.as_object() {
      for (key, value) in obj {
        let prefixed_key = format!("{}_{}", field, key);
        result.insert(prefixed_key, value.clone());
      }
    }

    serde_json::Value::Object(result)
  }

  fn unembed(&self, field: &str) -> serde_json::Value {
    let mut result = serde_json::Map::new();
    let prefix = format!("{}_", field);

    if let Some(obj) = self.as_object() {
      for (key, value) in obj {
        if let Some(stripped) = key.strip_prefix(&prefix) {
          result.insert(stripped.to_string(), value.clone());
        }
      }
    }

    serde_json::Value::Object(result)
  }
}
