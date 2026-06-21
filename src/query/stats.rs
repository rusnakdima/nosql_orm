use crate::query::Filter;
use std::collections::HashMap;
use std::sync::RwLock;

pub struct QueryStatistics {
  by_collection: RwLock<HashMap<String, CollectionStats>>,
}

#[derive(Debug, Clone)]
pub struct CollectionStats {
  pub total_queries: u64,
  pub slow_queries: u64,
  pub avg_duration_ms: f64,
  pub most_common_filters: Vec<FilterPattern>,
}

#[derive(Debug, Clone)]
pub struct FilterPattern {
  pub pattern: String,
  pub count: u64,
}

impl CollectionStats {
  pub fn new() -> Self {
    Self {
      total_queries: 0,
      slow_queries: 0,
      avg_duration_ms: 0.0,
      most_common_filters: Vec::new(),
    }
  }
}

impl Default for CollectionStats {
  fn default() -> Self {
    Self::new()
  }
}

impl QueryStatistics {
  pub fn new() -> Self {
    Self {
      by_collection: RwLock::new(HashMap::new()),
    }
  }

  pub fn record(&self, collection: &str, duration_ms: f64, filter: &Filter) {
    let mut guard = self.by_collection.write().unwrap();
    let stats = guard
      .entry(collection.to_string())
      .or_insert_with(CollectionStats::new);

    stats.total_queries += 1;
    if duration_ms > 100.0 {
      stats.slow_queries += 1;
    }

    let pattern_str = filter.to_string();
    if let Some(existing) = stats
      .most_common_filters
      .iter_mut()
      .find(|f| f.pattern == pattern_str)
    {
      existing.count += 1;
    } else {
      stats.most_common_filters.push(FilterPattern {
        pattern: pattern_str,
        count: 1,
      });
    }

    if stats.total_queries > 0 {
      stats.avg_duration_ms = (stats.avg_duration_ms * (stats.total_queries - 1) as f64
        + duration_ms)
        / stats.total_queries as f64;
    }
  }

  pub fn get_stats(&self, collection: &str) -> Option<CollectionStats> {
    self.by_collection.read().unwrap().get(collection).cloned()
  }
}

impl Default for QueryStatistics {
  fn default() -> Self {
    Self::new()
  }
}

impl Filter {
  pub fn to_string(&self) -> String {
    match self {
      Filter::Eq(field, val) => format!("{} = {}", field, val),
      Filter::Ne(field, val) => format!("{} != {}", field, val),
      Filter::Gt(field, val) => format!("{} > {}", field, val),
      Filter::Gte(field, val) => format!("{} >= {}", field, val),
      Filter::Lt(field, val) => format!("{} < {}", field, val),
      Filter::Lte(field, val) => format!("{} <= {}", field, val),
      Filter::In(field, vals) => format!("{} IN {:?}", field, vals),
      Filter::NotIn(field, vals) => format!("{} NOT IN {:?}", field, vals),
      Filter::ArrayContains(field, val) => format!("{} ARRAY_CONTAINS {:?}", field, val),
      Filter::Contains(field, sub) => format!("{} CONTAINS {:?}", field, sub),
      Filter::StartsWith(field, prefix) => format!("{} STARTS WITH {:?}", field, prefix),
      Filter::EndsWith(field, suffix) => format!("{} ENDS WITH {:?}", field, suffix),
      Filter::Like(field, pattern) => format!("{} LIKE {:?}", field, pattern),
      Filter::IsNull(field) => format!("{} IS NULL", field),
      Filter::IsNotNull(field) => format!("{} IS NOT NULL", field),
      Filter::Between(field, min, max) => format!("{} BETWEEN {:?} AND {:?}", field, min, max),
      Filter::And(filters) => {
        let inner = filters
          .iter()
          .map(|f| f.to_string())
          .collect::<Vec<_>>()
          .join(" AND ");
        format!("({})", inner)
      }
      Filter::Or(filters) => {
        let inner = filters
          .iter()
          .map(|f| f.to_string())
          .collect::<Vec<_>>()
          .join(" OR ");
        format!("({})", inner)
      }
      Filter::Not(inner) => format!("NOT ({})", inner.to_string()),
    }
  }
}
