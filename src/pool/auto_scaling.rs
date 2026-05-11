use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub struct AutoScalerConfig {
  pub min_size: usize,
  pub max_size: usize,
  pub target_utilization: f64,
  pub scale_up_threshold: f64,
  pub scale_down_threshold: f64,
  pub scale_up_cooldown_secs: u64,
  pub scale_down_cooldown_secs: u64,
}

impl Default for AutoScalerConfig {
  fn default() -> Self {
    Self {
      min_size: 5,
      max_size: 100,
      target_utilization: 0.7,
      scale_up_threshold: 0.8,
      scale_down_threshold: 0.3,
      scale_up_cooldown_secs: 30,
      scale_down_cooldown_secs: 120,
    }
  }
}

pub struct AutoScaler {
  config: AutoScalerConfig,
  current_size: AtomicUsize,
  last_scale_up: AtomicU64,
  last_scale_down: AtomicU64,
}

fn current_epoch_secs() -> u64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs()
}

impl AutoScaler {
  pub fn new(config: AutoScalerConfig) -> Self {
    Self {
      current_size: AtomicUsize::new(config.min_size),
      last_scale_up: AtomicU64::new(0),
      last_scale_down: AtomicU64::new(0),
      config,
    }
  }

  pub fn current_size(&self) -> usize {
    self.current_size.load(Ordering::Relaxed)
  }

  pub fn should_scale_up(&self, utilization: f64) -> bool {
    if utilization >= self.config.scale_up_threshold {
      let elapsed = current_epoch_secs() - self.last_scale_up.load(Ordering::Relaxed);
      elapsed >= self.config.scale_up_cooldown_secs
        && self.current_size.load(Ordering::Relaxed) < self.config.max_size
    } else {
      false
    }
  }

  pub fn should_scale_down(&self, utilization: f64) -> bool {
    if utilization <= self.config.scale_down_threshold {
      let elapsed = current_epoch_secs() - self.last_scale_down.load(Ordering::Relaxed);
      elapsed >= self.config.scale_down_cooldown_secs
        && self.current_size.load(Ordering::Relaxed) > self.config.min_size
    } else {
      false
    }
  }

  pub fn scale_up(&self) {
    let new_size = self.current_size.load(Ordering::Relaxed) + 1;
    if new_size <= self.config.max_size {
      self.current_size.store(new_size, Ordering::Relaxed);
      self
        .last_scale_up
        .store(current_epoch_secs(), Ordering::Relaxed);
    }
  }

  pub fn scale_down(&self) {
    let new_size = self.current_size.load(Ordering::Relaxed).saturating_sub(1);
    if new_size >= self.config.min_size {
      self.current_size.store(new_size, Ordering::Relaxed);
      self
        .last_scale_down
        .store(current_epoch_secs(), Ordering::Relaxed);
    }
  }
}
