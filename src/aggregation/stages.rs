use serde_json::Value;

pub struct MatchStage {
  pub filter: Value,
}

impl MatchStage {
  pub fn new(filter: Value) -> Self {
    Self { filter }
  }
}

pub struct GroupStage {
  pub id: Value,
  pub accumulators: std::collections::HashMap<String, Value>,
}

impl GroupStage {
  pub fn new(id: Value) -> Self {
    Self {
      id,
      accumulators: std::collections::HashMap::new(),
    }
  }

  pub fn sum(mut self, field: &str, expr: Value) -> Self {
    self
      .accumulators
      .insert(field.to_string(), serde_json::json!({ "$sum": expr }));
    self
  }

  pub fn avg(mut self, field: &str, expr: Value) -> Self {
    self
      .accumulators
      .insert(field.to_string(), serde_json::json!({ "$avg": expr }));
    self
  }

  pub fn min(mut self, field: &str, expr: Value) -> Self {
    self
      .accumulators
      .insert(field.to_string(), serde_json::json!({ "$min": expr }));
    self
  }

  pub fn max(mut self, field: &str, expr: Value) -> Self {
    self
      .accumulators
      .insert(field.to_string(), serde_json::json!({ "$max": expr }));
    self
  }
}

pub struct SortStage {
  pub field: String,
  pub ascending: bool,
}

pub struct LimitStage(pub u64);
pub struct SkipStage(pub u64);

pub struct ProjectStage {
  pub fields: std::collections::HashMap<String, Value>,
}

impl ProjectStage {
  pub fn new() -> Self {
    Self {
      fields: std::collections::HashMap::new(),
    }
  }

  pub fn include(mut self, field: &str) -> Self {
    self.fields.insert(field.to_string(), serde_json::json!(1));
    self
  }

  pub fn exclude(mut self, field: &str) -> Self {
    self.fields.insert(field.to_string(), serde_json::json!(0));
    self
  }
}
