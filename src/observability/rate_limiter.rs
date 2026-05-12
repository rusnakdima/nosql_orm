use crate::error::OrmResult;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct RateLimiterConfig {
  pub rate: f64,
  pub burst: u64,
  pub refill_rate: f64,
}

impl Default for RateLimiterConfig {
  fn default() -> Self {
    Self {
      rate: 100.0,
      burst: 10,
      refill_rate: 10.0,
    }
  }
}

impl RateLimiterConfig {
  pub fn with_rate(mut self, rate: f64) -> Self {
    self.rate = rate;
    self
  }

  pub fn with_burst(mut self, burst: u64) -> Self {
    self.burst = burst;
    self
  }

  pub fn with_refill_rate(mut self, refill_rate: f64) -> Self {
    self.refill_rate = refill_rate;
    self
  }
}

pub struct RateLimiterMetrics {
  pub requests_total: AtomicU64,
  pub allowed_total: AtomicU64,
  pub rejected_total: AtomicU64,
  pub tokens_acquired: AtomicU64,
}

impl RateLimiterMetrics {
  pub fn new() -> Self {
    Self {
      requests_total: AtomicU64::new(0),
      allowed_total: AtomicU64::new(0),
      rejected_total: AtomicU64::new(0),
      tokens_acquired: AtomicU64::new(0),
    }
  }

  pub fn record_request(&self) {
    self.requests_total.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_allowed(&self) {
    self.allowed_total.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_rejected(&self) {
    self.rejected_total.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_tokens(&self, tokens: u64) {
    self.tokens_acquired.fetch_add(tokens, Ordering::Relaxed);
  }

  pub fn requests(&self) -> u64 {
    self.requests_total.load(Ordering::Relaxed)
  }

  pub fn allowed(&self) -> u64 {
    self.allowed_total.load(Ordering::Relaxed)
  }

  pub fn rejected(&self) -> u64 {
    self.rejected_total.load(Ordering::Relaxed)
  }
}

impl Clone for RateLimiterMetrics {
  fn clone(&self) -> Self {
    Self {
      requests_total: AtomicU64::new(self.requests_total.load(Ordering::Relaxed)),
      allowed_total: AtomicU64::new(self.allowed_total.load(Ordering::Relaxed)),
      rejected_total: AtomicU64::new(self.rejected_total.load(Ordering::Relaxed)),
      tokens_acquired: AtomicU64::new(self.tokens_acquired.load(Ordering::Relaxed)),
    }
  }
}

impl Default for RateLimiterMetrics {
  fn default() -> Self {
    Self::new()
  }
}

pub struct RateLimiter {
  name: String,
  tokens: f64,
  max_tokens: f64,
  refill_rate: f64,
  last_refill: Instant,
  config: RateLimiterConfig,
  metrics: RateLimiterMetrics,
}

impl RateLimiter {
  pub fn new(name: impl Into<String>) -> Self {
    let config = RateLimiterConfig::default();
    Self {
      name: name.into(),
      tokens: config.burst as f64,
      max_tokens: config.burst as f64,
      refill_rate: config.refill_rate,
      last_refill: Instant::now(),
      config,
      metrics: RateLimiterMetrics::new(),
    }
  }

  pub fn with_config(mut self, config: RateLimiterConfig) -> Self {
    self.config = config.clone();
    self.tokens = self.config.burst as f64;
    self.max_tokens = self.config.burst as f64;
    self.refill_rate = self.config.refill_rate;
    self
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn metrics(&self) -> &RateLimiterMetrics {
    &self.metrics
  }

  fn refill(&mut self) {
    let elapsed = self.last_refill.elapsed().as_secs_f64();
    let tokens_to_add = elapsed * self.refill_rate;
    self.tokens = (self.tokens + tokens_to_add).min(self.max_tokens);
    self.last_refill = Instant::now();
  }

  pub fn acquire(&mut self, tokens: u64) -> bool {
    self.metrics.record_request();
    self.refill();

    if self.tokens >= tokens as f64 {
      self.tokens -= tokens as f64;
      self.metrics.record_allowed();
      self.metrics.record_tokens(tokens);
      tracing::debug!(
          limiter = %self.name,
          acquired = tokens,
          remaining = self.tokens as u64,
          "Rate limiter token acquired"
      );
      true
    } else {
      self.metrics.record_rejected();
      tracing::debug!(
          limiter = %self.name,
          requested = tokens,
          available = self.tokens as u64,
          "Rate limiter token rejected"
      );
      false
    }
  }

  pub async fn acquire_async(&mut self, tokens: u64) -> OrmResult<()> {
    let backoff_ms = 50;
    let max_attempts = 10;
    let mut attempts = 0;

    while !self.acquire(tokens) {
      attempts += 1;
      if attempts >= max_attempts {
        return Err(crate::error::OrmError::Connection(format!(
          "Rate limiter {}: exceeded max wait attempts",
          self.name
        )));
      }
      tokio::time::sleep(Duration::from_millis(backoff_ms * attempts as u64)).await;
    }

    Ok(())
  }

  pub fn available_tokens(&self) -> f64 {
    let elapsed = self.last_refill.elapsed().as_secs_f64();
    let tokens_to_add = elapsed * self.refill_rate;
    (self.tokens + tokens_to_add).min(self.max_tokens)
  }

  pub fn get_stats(&self) -> std::collections::HashMap<String, String> {
    let mut stats = std::collections::HashMap::new();
    stats.insert("name".to_string(), self.name.clone());
    stats.insert(
      "available_tokens".to_string(),
      (self.available_tokens() as u64).to_string(),
    );
    stats.insert(
      "max_tokens".to_string(),
      (self.max_tokens as u64).to_string(),
    );
    stats.insert("refill_rate".to_string(), self.refill_rate.to_string());
    stats.insert(
      "requests_total".to_string(),
      self.metrics.requests().to_string(),
    );
    stats.insert(
      "allowed_total".to_string(),
      self.metrics.allowed().to_string(),
    );
    stats.insert(
      "rejected_total".to_string(),
      self.metrics.rejected().to_string(),
    );
    stats
  }
}

impl Default for RateLimiter {
  fn default() -> Self {
    Self::new("default")
  }
}

impl std::fmt::Debug for RateLimiter {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("RateLimiter")
      .field("name", &self.name)
      .field("tokens", &self.tokens)
      .field("max_tokens", &self.max_tokens)
      .field("refill_rate", &self.refill_rate)
      .finish()
  }
}

pub struct DistributedRateLimiter {
  name: String,
  local_limiter: RateLimiter,
}

impl DistributedRateLimiter {
  pub fn new(name: impl Into<String>) -> Self {
    let name_str = name.into();
    Self {
      name: name_str.clone(),
      local_limiter: RateLimiter::new(name_str),
    }
  }

  pub fn with_config(mut self, config: RateLimiterConfig) -> Self {
    self.local_limiter = self.local_limiter.with_config(config);
    self
  }

  pub async fn acquire(&mut self, tokens: u64) -> OrmResult<()> {
    self.local_limiter.acquire_async(tokens).await
  }

  pub fn acquire_sync(&mut self, tokens: u64) -> bool {
    self.local_limiter.acquire(tokens)
  }

  pub fn get_stats(&self) -> std::collections::HashMap<String, String> {
    let mut stats = self.local_limiter.get_stats();
    stats.insert("distributed".to_string(), "true".to_string());
    stats
  }
}

impl Default for DistributedRateLimiter {
  fn default() -> Self {
    Self::new("default")
  }
}
