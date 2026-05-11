use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct ConnectionLifecycle {
  created_at: Instant,
  last_used: AtomicU64,
  use_count: AtomicU64,
  idle_time_ms: AtomicU64,
}

impl ConnectionLifecycle {
  pub fn new() -> Self {
    Self {
      created_at: Instant::now(),
      last_used: AtomicU64::new(current_timestamp_ms()),
      use_count: AtomicU64::new(0),
      idle_time_ms: AtomicU64::new(0),
    }
  }

  pub fn record_use(&self) {
    self
      .last_used
      .store(current_timestamp_ms(), Ordering::Relaxed);
    self.use_count.fetch_add(1, Ordering::Relaxed);
  }

  pub fn idle_time(&self) -> u64 {
    let last = self.last_used.load(Ordering::Relaxed);
    current_timestamp_ms() - last
  }

  pub fn age_ms(&self) -> u64 {
    self.created_at.elapsed().as_millis() as u64
  }

  pub fn use_count(&self) -> u64 {
    self.use_count.load(Ordering::Relaxed)
  }
}

fn current_timestamp_ms() -> u64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_millis() as u64
}

#[derive(Debug, Clone)]
pub enum ConnectionHealth {
  Healthy,
  Stale(u64),
  Aged(u64),
  Exhausted,
}

impl ConnectionHealth {
  pub fn assess(lifecycle: &ConnectionLifecycle, max_idle_ms: u64, max_age_ms: u64) -> Self {
    let idle = lifecycle.idle_time();
    let age = lifecycle.age_ms();

    if idle > max_idle_ms {
      ConnectionHealth::Stale(idle)
    } else if age > max_age_ms {
      ConnectionHealth::Aged(age)
    } else {
      ConnectionHealth::Healthy
    }
  }
}
