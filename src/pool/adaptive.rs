use crate::error::OrmResult;
use crate::pool::auto_scaling::AutoScaler;
use crate::pool::auto_scaling::AutoScalerConfig;
use crate::pool::pool_impl::{PoolConfig, PoolInner};
use crate::provider::DatabaseProvider;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{watch, OwnedSemaphorePermit};

const UTILIZATION_MULTIPLIER: u64 = 1000;

#[async_trait]
pub trait AdaptivePoolProvider: DatabaseProvider + Send + Sync {
  async fn on_scale_up(&self, _new_size: usize) -> OrmResult<()> {
    Ok(())
  }
  async fn on_scale_down(&self, _new_size: usize) -> OrmResult<()> {
    Ok(())
  }
}

pub struct AdaptivePool<P: AdaptivePoolProvider> {
  inner: Arc<PoolInner>,
  provider: Arc<P>,
  scaler: Arc<AutoScaler>,
  utilization: AtomicU64,
  #[allow(dead_code)]
  stop_ch: watch::Sender<()>,
}

impl<P: AdaptivePoolProvider> AdaptivePool<P> {
  pub fn with_scaler(provider: P, config: PoolConfig, scaler_config: AutoScalerConfig) -> Self {
    let scaler = Arc::new(AutoScaler::new(scaler_config));
    let (stop_ch, _) = watch::channel(());
    Self {
      inner: Arc::new(PoolInner::new(config.max_size)),
      provider: Arc::new(provider),
      scaler,
      utilization: AtomicU64::new(0),
      stop_ch,
    }
  }

  pub async fn acquire(&self, wait_for_available: bool) -> OrmResult<AdaptivePooled<P>> {
    let permit = self.inner.acquire(wait_for_available).await?;
    let utilization = self.calculate_utilization();
    self.utilization.store(utilization, Ordering::Relaxed);

    if self
      .scaler
      .should_scale_up(utilization as f64 / UTILIZATION_MULTIPLIER as f64)
    {
      let current = self.scaler.current_size();
      self.scaler.scale_up();
      let _ = self.provider.on_scale_up(current + 1).await;
    } else if self
      .scaler
      .should_scale_down(utilization as f64 / UTILIZATION_MULTIPLIER as f64)
    {
      let current = self.scaler.current_size();
      self.scaler.scale_down();
      let _ = self.provider.on_scale_down(current - 1).await;
    }

    Ok(AdaptivePooled {
      provider: self.provider.clone(),
      permit: Some(permit),
    })
  }

  fn calculate_utilization(&self) -> u64 {
    let available = self.inner.available.load(Ordering::Relaxed);
    let total = self.inner.total.load(Ordering::Relaxed);
    if total == 0 {
      0
    } else {
      ((1.0 - (available as f64 / total as f64)) * UTILIZATION_MULTIPLIER as f64) as u64
    }
  }

  pub fn current_size(&self) -> usize {
    self.scaler.current_size()
  }

  pub fn utilization(&self) -> f64 {
    self.utilization.load(Ordering::Relaxed) as f64 / UTILIZATION_MULTIPLIER as f64
  }
}

pub struct AdaptivePooled<P: AdaptivePoolProvider> {
  provider: Arc<P>,
  permit: Option<OwnedSemaphorePermit>,
}

impl<P: AdaptivePoolProvider> AdaptivePooled<P> {
  pub fn provider(&self) -> &P {
    &*self.provider
  }
}

impl<P: AdaptivePoolProvider> Drop for AdaptivePooled<P> {
  fn drop(&mut self) {
    self.permit.take();
  }
}

impl<P: AdaptivePoolProvider> Clone for AdaptivePool<P> {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
      provider: self.provider.clone(),
      scaler: self.scaler.clone(),
      utilization: AtomicU64::new(self.utilization.load(Ordering::Relaxed)),
      stop_ch: self.stop_ch.clone(),
    }
  }
}
