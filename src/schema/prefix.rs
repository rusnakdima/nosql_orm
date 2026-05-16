use std::sync::Arc;
use std::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct PrefixConfig {
  pub schema_prefix: Option<String>,
  pub env_prefix: Option<String>,
  pub tenant_prefix: Option<String>,
  pub global_prefix: Option<String>,
}

impl PrefixConfig {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn schema_prefix(mut self, prefix: &str) -> Self {
    self.schema_prefix = Some(prefix.to_string());
    self
  }

  pub fn env_prefix(mut self, prefix: &str) -> Self {
    self.env_prefix = Some(prefix.to_string());
    self
  }

  pub fn tenant_prefix(mut self, prefix: &str) -> Self {
    self.tenant_prefix = Some(prefix.to_string());
    self
  }

  pub fn global_prefix(mut self, prefix: &str) -> Self {
    self.global_prefix = Some(prefix.to_string());
    self
  }

  pub fn apply(&self, name: &str) -> String {
    let mut result = name.to_string();

    if let Some(ref global) = self.global_prefix {
      result = format!("{}{}", global, result);
    }
    if let Some(ref schema) = self.schema_prefix {
      result = format!("{}{}", schema, result);
    }
    if let Some(ref env) = self.env_prefix {
      result = format!("{}{}", env, result);
    }
    if let Some(ref tenant) = self.tenant_prefix {
      result = format!("{}{}", tenant, result);
    }

    result
  }

  pub fn strip(&self, prefixed_name: &str) -> String {
    let mut result = prefixed_name.to_string();

    if let Some(ref tenant) = self.tenant_prefix {
      if result.starts_with(tenant) {
        result = result[tenant.len()..].to_string();
      }
    }
    if let Some(ref env) = self.env_prefix {
      if result.starts_with(env) {
        result = result[env.len()..].to_string();
      }
    }
    if let Some(ref schema) = self.schema_prefix {
      if result.starts_with(schema) {
        result = result[schema.len()..].to_string();
      }
    }
    if let Some(ref global) = self.global_prefix {
      if result.starts_with(global) {
        result = result[global.len()..].to_string();
      }
    }

    result
  }
}

pub struct PrefixHolder {
  prefixes: Arc<RwLock<HashMap<String, String>>>,
}

impl PrefixHolder {
  pub fn new() -> Self {
    Self {
      prefixes: Arc::new(RwLock::new(HashMap::new())),
    }
  }

  pub fn with_config(config: PrefixConfig) -> Self {
    let mut prefixes = HashMap::new();
    if let Some(v) = config.global_prefix {
      prefixes.insert("global".to_string(), v);
    }
    if let Some(v) = config.schema_prefix {
      prefixes.insert("schema".to_string(), v);
    }
    if let Some(v) = config.env_prefix {
      prefixes.insert("env".to_string(), v);
    }
    if let Some(v) = config.tenant_prefix {
      prefixes.insert("tenant".to_string(), v);
    }
    Self {
      prefixes: Arc::new(RwLock::new(prefixes)),
    }
  }

  pub fn get_config(&self) -> PrefixConfig {
    let guard = match self.prefixes.read() {
      Ok(g) => g,
      Err(_) => {
        return PrefixConfig::default();
      }
    };
    let mut config = PrefixConfig::default();
    if let Some(v) = guard.get("global") {
      config.global_prefix = Some(v.clone());
    }
    if let Some(v) = guard.get("schema") {
      config.schema_prefix = Some(v.clone());
    }
    if let Some(v) = guard.get("env") {
      config.env_prefix = Some(v.clone());
    }
    if let Some(v) = guard.get("tenant") {
      config.tenant_prefix = Some(v.clone());
    }
    config
  }

  pub fn set_tenant(&self, tenant: Option<&str>) {
    if let Ok(mut prefixes) = self.prefixes.write() {
      if let Some(t) = tenant {
        prefixes.insert("tenant".to_string(), t.to_string());
      } else {
        prefixes.remove("tenant");
      }
    }
  }

  pub fn full_table_name(&self, name: &str) -> String {
    let guard = match self.prefixes.read() {
      Ok(g) => g,
      Err(_) => {
        return name.to_string();
      }
    };
    let mut result = name.to_string();
    for (_, v) in guard.iter() {
      result = format!("{}{}", v, result);
    }
    result
  }
}

impl Default for PrefixHolder {
  fn default() -> Self {
    Self::new()
  }
}

impl Clone for PrefixHolder {
  fn clone(&self) -> Self {
    Self {
      prefixes: self.prefixes.clone(),
    }
  }
}

use std::collections::HashMap;
pub type TablePrefix = PrefixConfig;
