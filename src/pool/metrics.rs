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
}

impl Default for PoolMetrics {
  fn default() -> Self {
    Self::new()
  }
}
