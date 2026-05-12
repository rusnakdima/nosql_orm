use crate::error::OrmResult;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[cfg(feature = "opentelemetry")]
use opentelemetry::global;
#[cfg(feature = "opentelemetry")]
use opentelemetry::trace::{Span, SpanKind, Tracer};

#[cfg(not(feature = "opentelemetry"))]
use tracing::Span as TracingSpan;

#[derive(Clone)]
pub struct TelemetryConfig {
  pub service_name: String,
  pub endpoint: Option<String>,
}

impl Default for TelemetryConfig {
  fn default() -> Self {
    Self {
      service_name: "nosql_orm".to_string(),
      endpoint: None,
    }
  }
}

#[derive(Clone)]
pub struct Telemetry {
  config: TelemetryConfig,
}

impl Telemetry {
  pub fn new(config: TelemetryConfig) -> Self {
    Self { config }
  }

  pub async fn trace_query(
    &self,
    collection: &str,
    filter: Option<&str>,
    fut: impl std::future::Future<Output = OrmResult<Vec<serde_json::Value>>>,
  ) -> OrmResult<Vec<serde_json::Value>> {
    let start = Instant::now();
    let result = fut.await;
    let duration = start.elapsed();
    self.record_query_metrics(collection, filter, duration, result.is_ok());
    result
  }

  pub async fn trace_transaction<F, T>(
    &self,
    tx_id: &str,
    fut: impl std::future::Future<Output = OrmResult<T>>,
  ) -> OrmResult<T> {
    let start = Instant::now();
    let result = fut.await;
    let duration = start.elapsed();
    self.record_transaction_metrics(tx_id, duration, result.is_ok());
    result
  }

  pub async fn trace_operation<F, T>(
    &self,
    operation_name: &str,
    attributes: HashMap<String, String>,
    fut: impl std::future::Future<Output = OrmResult<T>>,
  ) -> OrmResult<T> {
    let start = Instant::now();
    let result = fut.await;
    let duration = start.elapsed();
    self.record_operation_metrics(operation_name, attributes, duration, result.is_ok());
    result
  }

  #[cfg(feature = "opentelemetry")]
  pub fn start_span(&self, name: &str, kind: SpanKind) -> SpanBuilder {
    let _ = (name, kind);
    SpanBuilder { span: None }
  }

  #[cfg(not(feature = "opentelemetry"))]
  pub fn start_span(&self, name: &str, _kind: &str) -> SpanBuilder {
    let span = tracing::info_span!("{}", name);
    SpanBuilder {
      span: Some(SpanContext::Tracing(span)),
    }
  }

  fn record_query_metrics(
    &self,
    collection: &str,
    filter: Option<&str>,
    duration: Duration,
    success: bool,
  ) {
    tracing::debug!(
      collection = collection,
      filter = filter.unwrap_or("none"),
      duration_ms = duration.as_millis() as u64,
      success = success,
      "Query executed"
    );
  }

  fn record_transaction_metrics(&self, tx_id: &str, duration: Duration, success: bool) {
    tracing::debug!(
      tx_id = tx_id,
      duration_ms = duration.as_millis() as u64,
      success = success,
      "Transaction completed"
    );
  }

  fn record_operation_metrics(
    &self,
    operation_name: &str,
    attributes: HashMap<String, String>,
    duration: Duration,
    success: bool,
  ) {
    tracing::debug!(
      operation = operation_name,
      ?attributes,
      duration_ms = duration.as_millis() as u64,
      success = success,
      "Operation completed"
    );
  }
}

impl Default for Telemetry {
  fn default() -> Self {
    Self::new(TelemetryConfig::default())
  }
}

#[cfg(not(feature = "opentelemetry"))]
enum SpanContext {
  Tracing(TracingSpan),
}

#[cfg(not(feature = "opentelemetry"))]
pub struct SpanBuilder {
  span: Option<SpanContext>,
}

#[cfg(feature = "opentelemetry")]
pub struct SpanBuilder {
  span: Option<Box<dyn Span>>,
}

impl SpanBuilder {
  #[cfg(feature = "opentelemetry")]
  pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
    let _ = (key, value);
    self
  }

  #[cfg(not(feature = "opentelemetry"))]
  pub fn with_attribute(mut self, _key: &str, _value: &str) -> Self {
    self
  }

  #[cfg(feature = "opentelemetry")]
  pub fn add_event(mut self, name: &str) -> Self {
    let _ = name;
    self
  }

  #[cfg(not(feature = "opentelemetry"))]
  pub fn add_event(mut self, _name: &str) -> Self {
    self
  }

  #[cfg(feature = "opentelemetry")]
  pub fn set_status(mut self, status: &str) -> Self {
    let _ = status;
    self
  }

  #[cfg(not(feature = "opentelemetry"))]
  pub fn set_status(mut self, _status: &str) -> Self {
    self
  }

  pub fn end(self) {}
}

pub fn telemetry_closure<F, T>(telemetry: &Telemetry, name: &str, f: F) -> OrmResult<T>
where
  F: FnOnce() -> OrmResult<T>,
{
  let start = Instant::now();
  let result = f();
  let duration = start.elapsed();

  if let Err(ref e) = result {
    tracing::error!(
        operation = name,
        duration_ms = duration.as_millis() as u64,
        error = %e,
        "Operation failed"
    );
  } else {
    tracing::debug!(
      operation = name,
      duration_ms = duration.as_millis() as u64,
      "Operation completed"
    );
  }

  result
}
