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
  config: RwLock<PrefixConfig>,
}

impl PrefixHolder {
  pub fn new() -> Self {
    Self {
      config: RwLock::new(PrefixConfig::default()),
    }
  }

  pub fn with_config(config: PrefixConfig) -> Self {
    Self {
      config: RwLock::new(config),
    }
  }

  pub fn get_config(&self) -> PrefixConfig {
    self.config.read().unwrap().clone()
  }

  pub fn set_tenant(&self, tenant: Option<&str>) {
    let mut config = self.config.write().unwrap();
    config.tenant_prefix = tenant.map(|s| s.to_string());
  }

  pub fn full_table_name(&self, name: &str) -> String {
    self.config.read().unwrap().apply(name)
  }
}

impl Default for PrefixHolder {
  fn default() -> Self {
    Self::new()
  }
}

impl Clone for PrefixHolder {
  fn clone(&self) -> Self {
    Self::new()
  }
}

pub type TablePrefix = PrefixConfig;
