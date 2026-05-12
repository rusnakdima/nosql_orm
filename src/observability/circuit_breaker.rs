use crate::error::{OrmError, OrmResult};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
  Closed,
  Open,
  HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
  pub failure_threshold: u64,
  pub timeout_ms: u64,
  pub reset_interval_ms: u64,
}

impl Default for CircuitBreakerConfig {
  fn default() -> Self {
    Self {
      failure_threshold: 5,
      timeout_ms: 60000,
      reset_interval_ms: 30000,
    }
  }
}

impl CircuitBreakerConfig {
  pub fn with_failure_threshold(mut self, threshold: u64) -> Self {
    self.failure_threshold = threshold;
    self
  }

  pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
    self.timeout_ms = timeout_ms;
    self
  }

  pub fn with_reset_interval(mut self, interval_ms: u64) -> Self {
    self.reset_interval_ms = interval_ms;
    self
  }
}

pub struct CircuitBreakerMetrics {
  pub calls_total: AtomicU64,
  pub successes_total: AtomicU64,
  pub failures_total: AtomicU64,
  pub rejections_total: AtomicU64,
  pub state_changes: AtomicU64,
}

impl CircuitBreakerMetrics {
  pub fn new() -> Self {
    Self {
      calls_total: AtomicU64::new(0),
      successes_total: AtomicU64::new(0),
      failures_total: AtomicU64::new(0),
      rejections_total: AtomicU64::new(0),
      state_changes: AtomicU64::new(0),
    }
  }

  pub fn record_call(&self) {
    self.calls_total.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_success(&self) {
    self.successes_total.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_failure(&self) {
    self.failures_total.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_rejection(&self) {
    self.rejections_total.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_state_change(&self) {
    self.state_changes.fetch_add(1, Ordering::Relaxed);
  }

  pub fn calls(&self) -> u64 {
    self.calls_total.load(Ordering::Relaxed)
  }

  pub fn successes(&self) -> u64 {
    self.successes_total.load(Ordering::Relaxed)
  }

  pub fn failures(&self) -> u64 {
    self.failures_total.load(Ordering::Relaxed)
  }

  pub fn rejections(&self) -> u64 {
    self.rejections_total.load(Ordering::Relaxed)
  }

  pub fn state_changes_count(&self) -> u64 {
    self.state_changes.load(Ordering::Relaxed)
  }
}

impl Clone for CircuitBreakerMetrics {
  fn clone(&self) -> Self {
    Self {
      calls_total: AtomicU64::new(self.calls_total.load(Ordering::Relaxed)),
      successes_total: AtomicU64::new(self.successes_total.load(Ordering::Relaxed)),
      failures_total: AtomicU64::new(self.failures_total.load(Ordering::Relaxed)),
      rejections_total: AtomicU64::new(self.rejections_total.load(Ordering::Relaxed)),
      state_changes: AtomicU64::new(self.state_changes.load(Ordering::Relaxed)),
    }
  }
}

impl Default for CircuitBreakerMetrics {
  fn default() -> Self {
    Self::new()
  }
}

pub struct CircuitBreaker {
  name: String,
  state: CircuitState,
  failure_count: u64,
  last_failure_time: Option<Instant>,
  config: CircuitBreakerConfig,
  metrics: CircuitBreakerMetrics,
}

impl CircuitBreaker {
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      state: CircuitState::Closed,
      failure_count: 0,
      last_failure_time: None,
      config: CircuitBreakerConfig::default(),
      metrics: CircuitBreakerMetrics::new(),
    }
  }

  pub fn with_config(mut self, config: CircuitBreakerConfig) -> Self {
    self.config = config;
    self
  }

  pub fn state(&self) -> CircuitState {
    self.state
  }

  pub fn metrics(&self) -> &CircuitBreakerMetrics {
    &self.metrics
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  fn should_allow_request(&self) -> bool {
    match self.state {
      CircuitState::Closed => true,
      CircuitState::Open => {
        if let Some(last_failure) = self.last_failure_time {
          let timeout = Duration::from_millis(self.config.timeout_ms);
          if last_failure.elapsed() >= timeout {
            true
          } else {
            false
          }
        } else {
          false
        }
      }
      CircuitState::HalfOpen => true,
    }
  }

  fn transition_to(&mut self, new_state: CircuitState) {
    if self.state != new_state {
      tracing::debug!(
          circuit = %self.name,
          from = ?self.state,
          to = ?new_state,
          "Circuit breaker state change"
      );
      self.state = new_state;
      self.metrics.record_state_change();

      if new_state == CircuitState::HalfOpen {
        self.failure_count = 0;
      }
    }
  }

  pub async fn call<F, Fut, T>(&mut self, operation: F) -> OrmResult<T>
  where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = OrmResult<T>>,
  {
    self.metrics.record_call();

    if !self.should_allow_request() {
      self.metrics.record_rejection();
      return Err(OrmError::Connection(format!(
        "Circuit breaker {} is open",
        self.name
      )));
    }

    let start = Instant::now();
    let result = operation().await;
    let duration = start.elapsed();

    match result {
      Ok(value) => {
        self.metrics.record_success();
        self.failure_count = 0;

        if self.state == CircuitState::HalfOpen {
          self.transition_to(CircuitState::Closed);
        }

        tracing::debug!(
            circuit = %self.name,
            duration_ms = duration.as_millis() as u64,
            state = ?self.state,
            "Circuit breaker call succeeded"
        );

        Ok(value)
      }
      Err(e) => {
        self.metrics.record_failure();
        self.failure_count += 1;
        self.last_failure_time = Some(Instant::now());

        if self.failure_count >= self.config.failure_threshold {
          self.transition_to(CircuitState::Open);
        }

        tracing::warn!(
            circuit = %self.name,
            duration_ms = duration.as_millis() as u64,
            failure_count = self.failure_count,
            state = ?self.state,
            error = %e,
            "Circuit breaker call failed"
        );

        Err(e)
      }
    }
  }

  pub fn reset(&mut self) {
    self.state = CircuitState::Closed;
    self.failure_count = 0;
    self.last_failure_time = None;
    tracing::debug!(circuit = %self.name, "Circuit breaker reset");
  }

  pub fn get_stats(&self) -> HashMap<String, String> {
    let mut stats = HashMap::new();
    stats.insert("name".to_string(), self.name.clone());
    stats.insert("state".to_string(), format!("{:?}", self.state));
    stats.insert("failure_count".to_string(), self.failure_count.to_string());
    stats.insert(
      "last_failure".to_string(),
      self
        .last_failure_time
        .map(|t| format!("{:?}", t.elapsed()))
        .unwrap_or_default(),
    );
    stats.insert("calls_total".to_string(), self.metrics.calls().to_string());
    stats.insert(
      "successes_total".to_string(),
      self.metrics.successes().to_string(),
    );
    stats.insert(
      "failures_total".to_string(),
      self.metrics.failures().to_string(),
    );
    stats.insert(
      "rejections_total".to_string(),
      self.metrics.rejections().to_string(),
    );
    stats
  }
}

impl Default for CircuitBreaker {
  fn default() -> Self {
    Self::new("default")
  }
}

impl std::fmt::Debug for CircuitBreaker {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CircuitBreaker")
      .field("name", &self.name)
      .field("state", &self.state)
      .field("failure_count", &self.failure_count)
      .finish()
  }
}
