use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct QueryMetrics {
  pub query_count_total: AtomicU64,
  pub query_duration_ns: AtomicU64,
  pub slow_query_count: AtomicU64,
}

impl QueryMetrics {
  pub fn new() -> Self {
    Self {
      query_count_total: AtomicU64::new(0),
      query_duration_ns: AtomicU64::new(0),
      slow_query_count: AtomicU64::new(0),
    }
  }

  pub fn record_query(&self, duration_ns: u64, slow_threshold_ns: u64) {
    self.query_count_total.fetch_add(1, Ordering::Relaxed);
    self
      .query_duration_ns
      .fetch_add(duration_ns, Ordering::Relaxed);
    if duration_ns > slow_threshold_ns {
      self.slow_query_count.fetch_add(1, Ordering::Relaxed);
    }
  }

  pub fn total_queries(&self) -> u64 {
    self.query_count_total.load(Ordering::Relaxed)
  }

  pub fn total_duration_ns(&self) -> u64 {
    self.query_duration_ns.load(Ordering::Relaxed)
  }

  pub fn slow_query_count(&self) -> u64 {
    self.slow_query_count.load(Ordering::Relaxed)
  }

  pub fn avg_query_time_ms(&self) -> f64 {
    let count = self.query_count_total.load(Ordering::Relaxed);
    if count == 0 {
      0.0
    } else {
      let total_ns = self.query_duration_ns.load(Ordering::Relaxed);
      (total_ns as f64) / (count as f64) / 1_000_000.0
    }
  }
}

impl Default for QueryMetrics {
  fn default() -> Self {
    Self::new()
  }
}

pub struct PoolMetricsExporter {
  pool_name: String,
  metrics: Arc<crate::pool::PoolMetrics>,
}

impl PoolMetricsExporter {
  pub fn new(pool_name: impl Into<String>, metrics: Arc<crate::pool::PoolMetrics>) -> Self {
    Self {
      pool_name: pool_name.into(),
      metrics,
    }
  }

  pub fn export_prometheus(&self) -> String {
    self.metrics.export_prometheus(&self.pool_name)
  }
}

pub struct QueryMetricsExporter {
  query_metrics: Arc<QueryMetrics>,
  collection: String,
}

impl QueryMetricsExporter {
  pub fn new(collection: impl Into<String>, metrics: Arc<QueryMetrics>) -> Self {
    Self {
      collection: collection.into(),
      query_metrics: metrics,
    }
  }

  pub fn export_prometheus(&self) -> String {
    let total = self.query_metrics.total_queries();
    let duration_ns = self.query_metrics.total_duration_ns();
    let slow = self.query_metrics.slow_query_count();
    let avg_ms = self.query_metrics.avg_query_time_ms();

    format!(
      r#"# HELP nosql_query_count_total Total number of queries executed
# TYPE nosql_query_count_total counter
nosql_query_count_total{{collection="{col}"}} {total}
# HELP nosql_query_duration_ns_total Total query duration in nanoseconds
# TYPE nosql_query_duration_ns_total counter
nosql_query_duration_ns_total{{collection="{col}"}} {duration_ns}
# HELP nosql_query_slow_count_total Total number of slow queries
# TYPE nosql_query_slow_count_total counter
nosql_query_slow_count_total{{collection="{col}"}} {slow}
# HELP nosql_query_avg_time_ms Average query time in milliseconds
# TYPE nosql_query_avg_time_ms gauge
nosql_query_avg_time_ms{{collection="{col}"}} {avg_ms}
"#,
      col = self.collection
    )
  }
}

pub fn export_prometheus(metrics_exporters: Vec<Box<dyn MetricsExporter>>) -> String {
  metrics_exporters
    .iter()
    .map(|e| e.export())
    .collect::<Vec<_>>()
    .join("\n")
}

pub trait MetricsExporter: Send + Sync {
  fn export(&self) -> String;
}

impl MetricsExporter for PoolMetricsExporter {
  fn export(&self) -> String {
    self.export_prometheus()
  }
}

impl MetricsExporter for QueryMetricsExporter {
  fn export(&self) -> String {
    self.export_prometheus()
  }
}
