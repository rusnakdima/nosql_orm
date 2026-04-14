use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTextIndex {
  pub fields: Vec<String>,
  pub name: Option<String>,
  pub weights: Option<std::collections::HashMap<String, i32>>,
}

impl FullTextIndex {
  pub fn new(fields: Vec<String>) -> Self {
    Self {
      fields,
      name: None,
      weights: None,
    }
  }

  pub fn name(mut self, name: &str) -> Self {
    self.name = Some(name.to_string());
    self
  }

  pub fn weight(mut self, field: &str, weight: i32) -> Self {
    let mut weights = self.weights.unwrap_or_default();
    weights.insert(field.to_string(), weight);
    self.weights = Some(weights);
    self
  }
}

#[derive(Debug, Clone)]
pub struct FullTextSearch;

impl FullTextSearch {
  pub fn build_text_filter(query: &str) -> Value {
    serde_json::json!({
        "$text": { "$search": query }
    })
  }

  pub fn build_score_projection() -> Value {
    serde_json::json!({
        "score": { "$meta": "textScore" }
    })
  }

  pub fn build_text_score_sort() -> Value {
    serde_json::json!({
        "score": { "$meta": "textScore" }
    })
  }
}

pub struct InMemoryFullTextSearch {
  indexes: std::collections::HashMap<String, FullTextIndex>,
}

impl InMemoryFullTextSearch {
  pub fn new() -> Self {
    Self {
      indexes: std::collections::HashMap::new(),
    }
  }

  pub fn create_index(&mut self, collection: &str, index: FullTextIndex) {
    self.indexes.insert(collection.to_string(), index);
  }

  pub fn search(&self, docs: &[Value], query: &str, fields: &[String]) -> Vec<(Value, f64)> {
    let query_lower = query.to_lowercase();
    let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

    let mut results = Vec::new();

    for doc in docs {
      let mut score = 0.0;
      let mut highlights = Vec::new();

      for field in fields {
        if let Some(value) = doc.get(field).and_then(|v| v.as_str()) {
          let value_lower = value.to_lowercase();

          if value_lower.contains(&query_lower) {
            score += 10.0;
            highlights.push(value.to_string());
          }

          for term in &query_terms {
            if value_lower.contains(term) {
              score += 1.0;
            }
          }
        }
      }

      if score > 0.0 {
        results.push((doc.clone(), score));
      }
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results
  }
}

impl Default for InMemoryFullTextSearch {
  fn default() -> Self {
    Self::new()
  }
}
