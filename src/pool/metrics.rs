use std::sync::atomic::{AtomicU64, Ordering};

pub struct PoolMetrics {
  pub acquired_total: AtomicU64,
  pub released_total: AtomicU64,
  pub wait_time_total_ns: AtomicU64,
  pub wait_count: AtomicU64,
  pub scale_up_events: AtomicU64,
  pub scale_down_events: AtomicU64,
}

impl PoolMetrics {
  pub fn new() -> Self {
    Self {
      acquired_total: AtomicU64::new(0),
      released_total: AtomicU64::new(0),
      wait_time_total_ns: AtomicU64::new(0),
      wait_count: AtomicU64::new(0),
      scale_up_events: AtomicU64::new(0),
      scale_down_events: AtomicU64::new(0),
    }
  }

  pub fn record_acquire(&self) {
    self.acquired_total.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_release(&self) {
    self.released_total.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_wait(&self, wait_ns: u64) {
    self
      .wait_time_total_ns
      .fetch_add(wait_ns, Ordering::Relaxed);
    self.wait_count.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_scale_up(&self) {
    self.scale_up_events.fetch_add(1, Ordering::Relaxed);
  }

  pub fn record_scale_down(&self) {
    self.scale_down_events.fetch_add(1, Ordering::Relaxed);
  }

  pub fn avg_wait_time_ms(&self) -> f64 {
    let total = self.wait_time_total_ns.load(Ordering::Relaxed);
    let count = self.wait_count.load(Ordering::Relaxed);
    if count == 0 {
      0.0
    } else {
      (total as f64) / (count as f64) / 1_000_000.0
    }
  }

  pub fn total_acquires(&self) -> u64 {
    self.acquired_total.load(Ordering::Relaxed)
  }

  pub fn total_releases(&self) -> u64 {
    self.released_total.load(Ordering::Relaxed)
  }

  pub fn total_scale_ups(&self) -> u64 {
    self.scale_up_events.load(Ordering::Relaxed)
  }

  pub fn total_scale_downs(&self) -> u64 {
    self.scale_down_events.load(Ordering::Relaxed)
  }

  pub fn export_prometheus(&self, pool_name: &str) -> String {
    let acquired = self.acquired_total.load(Ordering::Relaxed);
    let released = self.released_total.load(Ordering::Relaxed);
    let wait_ns = self.wait_time_total_ns.load(Ordering::Relaxed);
    let wait_count = self.wait_count.load(Ordering::Relaxed);
    let scale_ups = self.scale_up_events.load(Ordering::Relaxed);
    let scale_downs = self.scale_down_events.load(Ordering::Relaxed);
    let avg_wait = self.avg_wait_time_ms();

    format!(
      r#"# HELP nosql_pool_acquired_total Total number of connections acquired
# TYPE nosql_pool_acquired_total counter
nosql_pool_acquired_total{{pool="{pool_name}"}} {acquired}
# HELP nosql_pool_released_total Total number of connections released
# TYPE nosql_pool_released_total counter
nosql_pool_released_total{{pool="{pool_name}"}} {released}
# HELP nosql_pool_wait_time_ns_total Total wait time in nanoseconds
# TYPE nosql_pool_wait_time_ns_total counter
nosql_pool_wait_time_ns_total{{pool="{pool_name}"}} {wait_ns}
# HELP nosql_pool_wait_count_total Total number of waits
# TYPE nosql_pool_wait_count_total counter
nosql_pool_wait_count_total{{pool="{pool_name}"}} {wait_count}
# HELP nosql_pool_scale_up_events_total Total scale up events
# TYPE nosql_pool_scale_up_events_total counter
nosql_pool_scale_up_events_total{{pool="{pool_name}"}} {scale_ups}
# HELP nosql_pool_scale_down_events_total Total scale down events
# TYPE nosql_pool_scale_down_events_total counter
nosql_pool_scale_down_events_total{{pool="{pool_name}"}} {scale_downs}
# HELP nosql_pool_avg_wait_time_ms Average wait time in milliseconds
# TYPE nosql_pool_avg_wait_time_ms gauge
nosql_pool_avg_wait_time_ms{{pool="{pool_name}"}} {avg_wait}
"#
    )
  }
}

impl Default for PoolMetrics {
  fn default() -> Self {
    Self::new()
  }
}
