use nosql_orm::observability::circuit_breaker::{
  CircuitBreaker, CircuitBreakerConfig, CircuitBreakerMetrics, CircuitState,
};
use nosql_orm::observability::rate_limiter::{RateLimiter, RateLimiterConfig, RateLimiterMetrics};
use std::time::Duration;

#[test]
fn test_circuit_breaker_config_default() {
  let config = CircuitBreakerConfig::default();
  assert_eq!(config.failure_threshold, 5);
  assert_eq!(config.timeout_ms, 60000);
  assert_eq!(config.reset_interval_ms, 30000);
}

#[test]
fn test_circuit_breaker_config_builder() {
  let config = CircuitBreakerConfig::default()
    .with_failure_threshold(10)
    .with_timeout(120000)
    .with_reset_interval(60000);

  assert_eq!(config.failure_threshold, 10);
  assert_eq!(config.timeout_ms, 120000);
  assert_eq!(config.reset_interval_ms, 60000);
}

#[test]
fn test_circuit_breaker_config_builder_chaining() {
  let config = CircuitBreakerConfig::default()
    .with_failure_threshold(3)
    .with_timeout(30000)
    .with_reset_interval(15000);

  assert_eq!(config.failure_threshold, 3);
  assert_eq!(config.timeout_ms, 30000);
  assert_eq!(config.reset_interval_ms, 15000);
}

#[test]
fn test_circuit_breaker_metrics_new() {
  let metrics = CircuitBreakerMetrics::new();
  assert_eq!(metrics.calls(), 0);
  assert_eq!(metrics.successes(), 0);
  assert_eq!(metrics.failures(), 0);
  assert_eq!(metrics.rejections(), 0);
  assert_eq!(metrics.state_changes(), 0);
}

#[test]
fn test_circuit_breaker_metrics_record_call() {
  let metrics = CircuitBreakerMetrics::new();
  metrics.record_call();
  assert_eq!(metrics.calls(), 1);

  metrics.record_call();
  metrics.record_call();
  assert_eq!(metrics.calls(), 3);
}

#[test]
fn test_circuit_breaker_metrics_record_success() {
  let metrics = CircuitBreakerMetrics::new();
  metrics.record_success();
  metrics.record_success();
  assert_eq!(metrics.successes(), 2);
}

#[test]
fn test_circuit_breaker_metrics_record_failure() {
  let metrics = CircuitBreakerMetrics::new();
  metrics.record_failure();
  assert_eq!(metrics.failures(), 1);
}

#[test]
fn test_circuit_breaker_metrics_record_rejection() {
  let metrics = CircuitBreakerMetrics::new();
  metrics.record_rejection();
  assert_eq!(metrics.rejections(), 1);
}

#[test]
fn test_circuit_breaker_metrics_record_state_change() {
  let metrics = CircuitBreakerMetrics::new();
  metrics.record_state_change();
  metrics.record_state_change();
  assert_eq!(metrics.state_changes(), 2);
}

#[test]
fn test_circuit_breaker_metrics_concurrent() {
  use std::sync::Arc;
  use std::thread;

  let metrics = Arc::new(CircuitBreakerMetrics::new());
  let mut handles = vec![];

  for _ in 0..10 {
    let m = metrics.clone();
    handles.push(thread::spawn(move || {
      for _ in 0..100 {
        m.record_call();
        m.record_success();
      }
    }));
  }

  for h in handles {
    h.join().unwrap();
  }

  assert_eq!(metrics.calls(), 1000);
  assert_eq!(metrics.successes(), 1000);
}

#[test]
fn test_circuit_state_equality() {
  assert_eq!(CircuitState::Closed, CircuitState::Closed);
  assert_eq!(CircuitState::Open, CircuitState::Open);
  assert_eq!(CircuitState::HalfOpen, CircuitState::HalfOpen);
  assert_ne!(CircuitState::Closed, CircuitState::Open);
  assert_ne!(CircuitState::Open, CircuitState::HalfOpen);
}

#[test]
fn test_circuit_state_debug() {
  assert_eq!(format!("{:?}", CircuitState::Closed), "Closed");
  assert_eq!(format!("{:?}", CircuitState::Open), "Open");
  assert_eq!(format!("{:?}", CircuitState::HalfOpen), "HalfOpen");
}

fn create_test_breaker() -> CircuitBreaker {
  CircuitBreaker::with_config(
    CircuitBreakerConfig::default()
      .with_failure_threshold(3)
      .with_timeout(1000)
      .with_reset_interval(100),
  )
}

#[tokio::test]
async fn test_circuit_breaker_initial_state_closed() {
  let breaker = create_test_breaker();
  assert_eq!(breaker.state(), CircuitState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_allows_requests_in_closed_state() {
  let breaker = create_test_breaker();
  assert!(breaker.should_allow_request().await);
}

#[tokio::test]
async fn test_circuit_breaker_state_transitions_on_failure() {
  let mut breaker = create_test_breaker();

  for _ in 0..3 {
    let _ = breaker.record_failure().await;
  }

  assert_eq!(breaker.state(), CircuitState::Open);
}

#[tokio::test]
async fn test_circuit_breaker_rejects_requests_in_open_state() {
  let mut breaker = create_test_breaker();

  for _ in 0..3 {
    let _ = breaker.record_failure().await;
  }

  assert_eq!(breaker.state(), CircuitState::Open);
  assert!(!breaker.should_allow_request().await);
}

#[tokio::test]
async fn test_circuit_breaker_half_open_after_timeout() {
  let mut breaker = create_test_breaker();

  for _ in 0..3 {
    let _ = breaker.record_failure().await;
  }

  assert_eq!(breaker.state(), CircuitState::Open);

  tokio::time::sleep(Duration::from_millis(1500)).await;

  assert!(breaker.should_allow_request().await);
}

#[tokio::test]
async fn test_circuit_breaker_reset() {
  let mut breaker = create_test_breaker();

  for _ in 0..3 {
    let _ = breaker.record_failure().await;
  }

  assert_eq!(breaker.state(), CircuitState::Open);

  breaker.reset();

  assert_eq!(breaker.state(), CircuitState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_get_stats() {
  let mut breaker = create_test_breaker();

  for _ in 0..5 {
    let _ = breaker.record_call().await;
    let _ = breaker.record_success().await;
  }

  for _ in 0..2 {
    let _ = breaker.record_failure().await;
  }

  let stats = breaker.get_stats();
  assert_eq!(stats.calls, 7);
  assert_eq!(stats.successes, 5);
  assert_eq!(stats.failures, 2);
}

#[test]
fn test_rate_limiter_config_default() {
  let config = RateLimiterConfig::default();
  assert_eq!(config.rate, 100.0);
  assert_eq!(config.burst, 10);
  assert_eq!(config.refill_rate, 10.0);
}

#[test]
fn test_rate_limiter_config_builder() {
  let config = RateLimiterConfig::default()
    .with_rate(50.0)
    .with_burst(20)
    .with_refill_rate(5.0);

  assert_eq!(config.rate, 50.0);
  assert_eq!(config.burst, 20);
  assert_eq!(config.refill_rate, 5.0);
}

#[test]
fn test_rate_limiter_metrics_new() {
  let metrics = RateLimiterMetrics::new();
  assert_eq!(metrics.requests(), 0);
  assert_eq!(metrics.allowed(), 0);
  assert_eq!(metrics.rejected(), 0);
}

#[test]
fn test_rate_limiter_metrics_record_request() {
  let metrics = RateLimiterMetrics::new();
  metrics.record_request();
  assert_eq!(metrics.requests(), 1);
}

#[test]
fn test_rate_limiter_metrics_record_allowed() {
  let metrics = RateLimiterMetrics::new();
  metrics.record_allowed();
  metrics.record_allowed();
  assert_eq!(metrics.allowed(), 2);
}

#[test]
fn test_rate_limiter_metrics_record_rejected() {
  let metrics = RateLimiterMetrics::new();
  metrics.record_rejected();
  assert_eq!(metrics.rejected(), 1);
}

#[test]
fn test_rate_limiter_metrics_record_tokens() {
  let metrics = RateLimiterMetrics::new();
  metrics.record_tokens(5);
  assert_eq!(metrics.tokens_acquired(), 5);
}

#[test]
fn test_rate_limiter_metrics_clone() {
  let metrics = RateLimiterMetrics::new();
  metrics.record_request();
  metrics.record_allowed();

  let cloned = metrics.clone();
  assert_eq!(cloned.requests(), 1);
  assert_eq!(cloned.allowed(), 1);
}

fn create_test_rate_limiter() -> RateLimiter {
  RateLimiter::with_config(
    RateLimiterConfig::default()
      .with_rate(10.0)
      .with_burst(5)
      .with_refill_rate(2.0),
  )
}

#[test]
fn test_rate_limiter_initial_tokens() {
  let limiter = create_test_rate_limiter();
  let tokens = limiter.available_tokens();
  assert!(tokens >= 5.0);
}

#[test]
fn test_rate_limiter_acquire_decrements_tokens() {
  let mut limiter = create_test_rate_limiter();
  let initial = limiter.available_tokens();

  let acquired = limiter.acquire(1);
  assert!(acquired);

  let after = limiter.available_tokens();
  assert!(after < initial);
}

#[test]
fn test_rate_limiter_acquire_returns_true_when_tokens_available() {
  let mut limiter = create_test_rate_limiter();
  let result = limiter.acquire(1);
  assert!(result);
}

#[test]
fn test_rate_limiter_available_tokens() {
  let limiter = create_test_rate_limiter();
  let tokens = limiter.available_tokens();
  assert!(tokens >= 0.0);
}

#[test]
fn test_rate_limiter_get_stats() {
  let mut limiter = create_test_rate_limiter();

  for _ in 0..3 {
    let _ = limiter.acquire(1);
  }

  let stats = limiter.get_stats();
  assert_eq!(stats.get("requests_total").map(|s| s.as_str()), Some("3"));
  assert_eq!(stats.get("allowed_total").map(|s| s.as_str()), Some("3"));
}

#[test]
fn test_rate_limiter_multiple_acquires() {
  let mut limiter = create_test_rate_limiter();

  for _ in 0..3 {
    let result = limiter.acquire(1);
    assert!(result);
  }
}

#[test]
fn test_rate_limiter_config_chaining() {
  let config = RateLimiterConfig::default()
    .with_rate(200.0)
    .with_burst(50)
    .with_refill_rate(25.0);

  assert_eq!(config.rate, 200.0);
  assert_eq!(config.burst, 50);
  assert_eq!(config.refill_rate, 25.0);
}
