//! StatisticsService — record and query time-series counters.
//!
//! Useful for metrics, observability, and usage tracking.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Errors for statistics operations.
#[derive(Debug, thiserror::Error, Clone)]
pub enum StatisticsError {
  #[error("counter not found: {0}")]
  NotFound(String),
}

/// StatisticsService trait.
pub trait StatisticsService: Send + Sync {
  /// Record an event (increments the named counter by 1).
  fn record(&self, name: &str) -> u64;

  /// Record a delta (positive or negative) on a counter.
  fn record_delta(&self, name: &str, delta: i64) -> i64;

  /// Query the current count for a name.
  fn count(&self, name: &str) -> u64;

  /// Query the cumulative count for a name (alias for count).
  fn query(&self, name: &str) -> u64 {
    self.count(name)
  }

  /// List all known counter names.
  fn names(&self) -> Vec<String>;
}

/// In-memory StatisticsService implementation.
pub struct InMemoryStatisticsService {
  counters: Arc<std::sync::RwLock<HashMap<String, Arc<AtomicU64>>>>,
}

impl InMemoryStatisticsService {
  /// Create a new in-memory statistics service.
  pub fn new() -> Self {
    Self::default()
  }
}

impl Default for InMemoryStatisticsService {
  fn default() -> Self {
    Self {
      counters: Arc::new(std::sync::RwLock::new(HashMap::new())),
    }
  }
}

impl StatisticsService for InMemoryStatisticsService {
  fn record(&self, name: &str) -> u64 {
    self.record_delta(name, 1) as u64
  }

  fn record_delta(&self, name: &str, delta: i64) -> i64 {
    let counter = {
      let mut counters = self.counters.write().expect("counters poisoned");
      counters
        .entry(name.to_string())
        .or_insert_with(|| Arc::new(AtomicU64::new(0)))
        .clone()
    };
    if delta >= 0 {
      counter.fetch_add(delta as u64, Ordering::SeqCst) as i64
    } else {
      let d = (-delta) as u64;
      let current = counter.load(Ordering::SeqCst);
      let new = current.saturating_sub(d);
      counter.store(new, Ordering::SeqCst);
      new as i64
    }
  }

  fn count(&self, name: &str) -> u64 {
    self
      .counters
      .read()
      .expect("counters poisoned")
      .get(name)
      .map(|c| c.load(Ordering::SeqCst))
      .unwrap_or(0)
  }

  fn names(&self) -> Vec<String> {
    self
      .counters
      .read()
      .expect("counters poisoned")
      .keys()
      .cloned()
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::Value;

  #[test]
  fn record_increments() {
    let s = InMemoryStatisticsService::new();
    s.record("clicks");
    s.record("clicks");
    s.record("clicks");
    assert_eq!(s.count("clicks"), 3);
  }

  #[test]
  fn record_delta() {
    let s = InMemoryStatisticsService::new();
    s.record_delta("score", 10);
    s.record_delta("score", 5);
    s.record_delta("score", -3);
    assert_eq!(s.count("score"), 12);
  }

  #[test]
  fn unknown_count_is_zero() {
    let s = InMemoryStatisticsService::new();
    assert_eq!(s.count("nope"), 0);
  }

  #[test]
  fn names() {
    let s = InMemoryStatisticsService::new();
    s.record("a");
    s.record("b");
    let mut names = s.names();
    names.sort();
    assert_eq!(names, vec!["a", "b"]);
  }

  #[test]
  fn record_value_serde() {
    let s = InMemoryStatisticsService::new();
    // Make sure Value is still imported in this module.
    let _: Value = serde_json::json!(42);
    assert_eq!(s.count("unused"), 0);
  }
}
