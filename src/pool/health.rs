use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch;

#[derive(Debug, Clone)]
pub struct HealthStatus {
  pub healthy: bool,
  pub latency_ms: Option<u64>,
  pub error: Option<String>,
  pub last_check: DateTime<Utc>,
}

impl HealthStatus {
  pub fn healthy(latency_ms: u64) -> Self {
    Self {
      healthy: true,
      latency_ms: Some(latency_ms),
      error: None,
      last_check: Utc::now(),
    }
  }

  pub fn unhealthy(error: String) -> Self {
    Self {
      healthy: false,
      latency_ms: None,
      error: Some(error),
      last_check: Utc::now(),
    }
  }
}

#[async_trait]
pub trait HealthCheckable {
  async fn check_health(&self) -> OrmResult<HealthStatus>;
}

pub struct HealthChecker {
  interval_secs: u64,
  threshold_ms: u64,
  unhealthy_threshold: u32,
  stop_ch: watch::Sender<()>,
  handle: Option<tokio::task::JoinHandle<()>>,
}

impl HealthChecker {
  pub fn new(interval_secs: u64, threshold_ms: u64) -> Self {
    let (stop_ch, _) = watch::channel(());
    Self {
      interval_secs,
      threshold_ms,
      unhealthy_threshold: 3,
      stop_ch,
      handle: None,
    }
  }

  pub fn threshold_ms(mut self, ms: u64) -> Self {
    self.threshold_ms = ms;
    self
  }

  pub fn unhealthy_threshold(mut self, n: u32) -> Self {
    self.unhealthy_threshold = n;
    self
  }

  pub fn stop(&self) {
    let _ = self.stop_ch.send(());
  }

  pub async fn shutdown(&mut self) -> OrmResult<()> {
    self.stop();
    if let Some(handle) = self.handle.take() {
      match handle.await {
        Ok(_) => Ok(()),
        Err(e) => Err(OrmError::Internal(format!(
          "Health check task join error: {}",
          e
        ))),
      }
    } else {
      Ok(())
    }
  }

  pub async fn start_checking<P>(&mut self, provider: Arc<P>) -> OrmResult<()>
  where
    P: DatabaseProvider + HealthCheckable,
  {
    let interval_secs = self.interval_secs;
    let threshold_ms = self.threshold_ms;
    let unhealthy_threshold = self.unhealthy_threshold;
    let mut stop_rx = self.stop_ch.subscribe();

    let handle = tokio::spawn(async move {
      let mut consecutive_failures = 0u32;
      let mut healthy = true;

      loop {
        tokio::select! {
          _ = stop_rx.changed() => {
            break;
          }
          _ = tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)) => {
            let start = Instant::now();
            let result = <P as HealthCheckable>::check_health(provider.as_ref()).await;
            let elapsed = start.elapsed().as_millis() as u64;

            match result {
              Ok(status) => {
                if status.healthy && elapsed <= threshold_ms {
                  consecutive_failures = 0;
                  if !healthy {
                    healthy = true;
                  }
                } else if elapsed > threshold_ms {
                  consecutive_failures += 1;
                }
              }
              Err(_) => {
                consecutive_failures += 1;
              }
            }

            if consecutive_failures >= unhealthy_threshold && healthy {
              healthy = false;
            }
          }
        }
      }
    });

    self.handle = Some(handle);
    Ok(())
  }
}

impl Default for HealthChecker {
  fn default() -> Self {
    Self::new(30, 5000)
  }
}

impl Drop for HealthChecker {
  fn drop(&mut self) {
    self.stop();
    if let Some(handle) = self.handle.take() {
      handle.abort();
    }
  }
}
