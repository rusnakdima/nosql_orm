use crate::admin_types::IndexInfo;
use crate::query::Filter;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum OptimizationHint {
  UseIndex(String),
  ForceJoin(String),
  AvoidIndex(String),
  OptimizeLimit(usize),
  UseCoveringIndex(String),
  DisableSort(bool),
  BatchSize(usize),
}

#[derive(Debug, Clone)]
pub struct QueryHint {
  pub hints: Vec<OptimizationHint>,
  pub reason: String,
}

impl QueryHint {
  pub fn new() -> Self {
    Self {
      hints: Vec::new(),
      reason: String::new(),
    }
  }

  pub fn use_index(mut self, idx: &str) -> Self {
    self.hints.push(OptimizationHint::UseIndex(idx.to_string()));
    self
  }

  pub fn force_join(mut self, join: &str) -> Self {
    self
      .hints
      .push(OptimizationHint::ForceJoin(join.to_string()));
    self
  }

  pub fn avoid_index(mut self, idx: &str) -> Self {
    self
      .hints
      .push(OptimizationHint::AvoidIndex(idx.to_string()));
    self
  }

  pub fn optimize_limit(mut self, limit: usize) -> Self {
    self.hints.push(OptimizationHint::OptimizeLimit(limit));
    self
  }

  pub fn use_covering_index(mut self, idx: &str) -> Self {
    self
      .hints
      .push(OptimizationHint::UseCoveringIndex(idx.to_string()));
    self
  }

  pub fn disable_sort(mut self, disable: bool) -> Self {
    self.hints.push(OptimizationHint::DisableSort(disable));
    self
  }

  pub fn batch_size(mut self, size: usize) -> Self {
    self.hints.push(OptimizationHint::BatchSize(size));
    self
  }

  pub fn reason(mut self, r: &str) -> Self {
    self.reason = r.to_string();
    self
  }
}

impl Default for QueryHint {
  fn default() -> Self {
    Self::new()
  }
}

pub struct QueryAnalyzer {
  index_info: HashMap<String, Vec<IndexInfo>>,
}

impl QueryAnalyzer {
  pub fn new() -> Self {
    Self {
      index_info: HashMap::new(),
    }
  }

  pub fn register_index(&mut self, collection: &str, idx: IndexInfo) {
    self
      .index_info
      .entry(collection.to_string())
      .or_default()
      .push(idx);
  }

  pub fn analyze(&self, collection: &str, filter: &Filter) -> QueryHint {
    let mut hint = QueryHint::new();

    if let Some(indexes) = self.index_info.get(collection) {
      for idx in indexes {
        if self.index_covers_filter(idx, filter) {
          hint
            .hints
            .push(OptimizationHint::UseIndex(idx.name.clone()));
          hint.reason = format!("Index {} covers filter", idx.name);
          break;
        }
      }
    }

    if let Filter::And(filters) = filter {
      for f in filters {
        if matches_sort(f) {
          hint.hints.push(OptimizationHint::DisableSort(false));
          break;
        }
      }
    }

    hint
  }

  fn index_covers_filter(&self, idx: &IndexInfo, filter: &Filter) -> bool {
    let filter_fields = extract_filter_fields(filter);
    for field in &filter_fields {
      if !idx.fields.contains(field) {
        return false;
      }
    }
    !filter_fields.is_empty()
  }
}

impl Default for QueryAnalyzer {
  fn default() -> Self {
    Self::new()
  }
}

fn extract_filter_fields(filter: &Filter) -> Vec<String> {
  match filter {
    Filter::Eq(field, _) => vec![field.clone()],
    Filter::Ne(field, _) => vec![field.clone()],
    Filter::Gt(field, _) => vec![field.clone()],
    Filter::Gte(field, _) => vec![field.clone()],
    Filter::Lt(field, _) => vec![field.clone()],
    Filter::Lte(field, _) => vec![field.clone()],
    Filter::In(field, _) => vec![field.clone()],
    Filter::NotIn(field, _) => vec![field.clone()],
    Filter::ArrayContains(field, _) => vec![field.clone()],
    Filter::Contains(field, _) => vec![field.clone()],
    Filter::StartsWith(field, _) => vec![field.clone()],
    Filter::EndsWith(field, _) => vec![field.clone()],
    Filter::Like(field, _) => vec![field.clone()],
    Filter::IsNull(field) => vec![field.clone()],
    Filter::IsNotNull(field) => vec![field.clone()],
    Filter::Between(field, _, _) => vec![field.clone()],
    Filter::And(filters) => {
      let mut fields = Vec::new();
      for f in filters {
        fields.extend(extract_filter_fields(f));
      }
      fields
    }
    Filter::Or(filters) => {
      let mut fields = Vec::new();
      for f in filters {
        fields.extend(extract_filter_fields(f));
      }
      fields
    }
    Filter::Not(inner) => extract_filter_fields(inner),
  }
}

fn matches_sort(_filter: &Filter) -> bool {
  false
}
