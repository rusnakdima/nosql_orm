use crate::admin_types::IndexInfo;
use crate::query::hints::{OptimizationHint, QueryAnalyzer, QueryHint};
use crate::query::Filter;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct OptimizedQuery {
  pub original: String,
  pub optimized: String,
  pub cost_estimate: f64,
  pub hints: Vec<OptimizationHint>,
  pub warnings: Vec<String>,
}

pub struct QueryOptimizer {
  analyzers: HashMap<String, QueryAnalyzer>,
  rule_based_optimizer: bool,
}

impl QueryOptimizer {
  pub fn new() -> Self {
    Self {
      analyzers: HashMap::new(),
      rule_based_optimizer: true,
    }
  }

  pub fn register_collection(&mut self, collection: &str, indexes: Vec<IndexInfo>) {
    let mut analyzer = QueryAnalyzer::new();
    for idx in indexes {
      analyzer.register_index(collection, idx);
    }
    self.analyzers.insert(collection.to_string(), analyzer);
  }

  pub fn optimize(&self, collection: &str, filter: &Filter) -> OptimizedQuery {
    let mut warnings = Vec::new();
    let optimized_filter = self.apply_rules(collection, filter, &mut warnings);

    let hint = self
      .analyzers
      .get(collection)
      .map(|a| a.analyze(collection, &optimized_filter))
      .unwrap_or_else(|| QueryHint::new());

    OptimizedQuery {
      original: filter.to_string(),
      optimized: optimized_filter.to_string(),
      cost_estimate: self.estimate_cost(&optimized_filter),
      hints: hint.hints,
      warnings,
    }
  }

  fn apply_rules(&self, _collection: &str, filter: &Filter, _warnings: &mut Vec<String>) -> Filter {
    if let Filter::And(inner) = filter {
      if inner.is_empty() {
        return Filter::And(vec![]);
      }
    }
    filter.clone()
  }

  fn estimate_cost(&self, filter: &Filter) -> f64 {
    match filter {
      Filter::Eq(_, _) => 1.0,
      Filter::In(_, v) => v.len() as f64 * 1.5,
      Filter::And(inner) => inner.len() as f64 * 2.0,
      Filter::Or(inner) => inner.len() as f64 * 3.0,
      _ => 10.0,
    }
  }
}

impl Default for QueryOptimizer {
  fn default() -> Self {
    Self::new()
  }
}
