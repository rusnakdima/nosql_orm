use crate::query::Filter;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
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
    // A poisoned lock means some other thread panicked while holding it.
    // The statistics map is rebuilt from the recorded events, so the guard
    // is recovered rather than propagating the panic into every subsequent
    // query.
    let mut guard = self
      .by_collection
      .write()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    let stats = guard.entry(collection.to_string()).or_default();

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
    self
      .by_collection
      .read()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .get(collection)
      .cloned()
  }
}

impl Default for QueryStatistics {
  fn default() -> Self {
    Self::new()
  }
}

impl Display for Filter {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Filter::Eq(field, val) => write!(f, "{} = {}", field, val),
      Filter::Ne(field, val) => write!(f, "{} != {}", field, val),
      Filter::Gt(field, val) => write!(f, "{} > {}", field, val),
      Filter::Gte(field, val) => write!(f, "{} >= {}", field, val),
      Filter::Lt(field, val) => write!(f, "{} < {}", field, val),
      Filter::Lte(field, val) => write!(f, "{} <= {}", field, val),
      Filter::In(field, vals) => write!(f, "{} IN {:?}", field, vals),
      Filter::NotIn(field, vals) => write!(f, "{} NOT IN {:?}", field, vals),
      Filter::ArrayContains(field, val) => write!(f, "{} ARRAY_CONTAINS {:?}", field, val),
      Filter::Contains(field, sub) => write!(f, "{} CONTAINS {:?}", field, sub),
      Filter::StartsWith(field, prefix) => write!(f, "{} STARTS WITH {:?}", field, prefix),
      Filter::EndsWith(field, suffix) => write!(f, "{} ENDS WITH {:?}", field, suffix),
      Filter::Like(field, pattern) => write!(f, "{} LIKE {:?}", field, pattern),
      Filter::IsNull(field) => write!(f, "{} IS NULL", field),
      Filter::IsNotNull(field) => write!(f, "{} IS NOT NULL", field),
      Filter::Between(field, min, max) => write!(f, "{} BETWEEN {:?} AND {:?}", field, min, max),
      Filter::And(filters) => {
        let inner = filters
          .iter()
          .map(|f| format!("{}", f))
          .collect::<Vec<_>>()
          .join(" AND ");
        write!(f, "({})", inner)
      }
      Filter::Or(filters) => {
        let inner = filters
          .iter()
          .map(|f| format!("{}", f))
          .collect::<Vec<_>>()
          .join(" OR ");
        write!(f, "({})", inner)
      }
      Filter::Not(inner) => write!(f, "NOT ({})", inner),
    }
  }
}
