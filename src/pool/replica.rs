use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ReplicaConfig {
  pub primary_url: String,
  pub replica_urls: Vec<String>,
  pub read_selection: ReadSelection,
  pub failover: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadSelection {
  RoundRobin,
  LeastLatency,
  Random,
}

impl Default for ReadSelection {
  fn default() -> Self {
    ReadSelection::RoundRobin
  }
}

pub struct ReplicaPool<P: DatabaseProvider> {
  primary: Arc<P>,
  replicas: Vec<Arc<P>>,
  config: ReplicaConfig,
  round_robin_index: std::sync::atomic::AtomicUsize,
}

impl<P: DatabaseProvider> ReplicaPool<P> {
  pub fn new(primary: Arc<P>, config: ReplicaConfig) -> Self {
    Self {
      primary,
      replicas: Vec::new(),
      config,
      round_robin_index: std::sync::atomic::AtomicUsize::new(0),
    }
  }

  pub fn add_replica(&mut self, provider: Arc<P>) {
    self.replicas.push(provider);
  }

  pub fn select_replica(&self) -> Arc<P> {
    match self.config.read_selection {
      ReadSelection::RoundRobin => {
        let idx = self
          .round_robin_index
          .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let replica_idx = idx % self.replicas.len().max(1);
        self
          .replicas
          .get(replica_idx)
          .cloned()
          .unwrap_or_else(|| self.primary.clone())
      }
      ReadSelection::Random => {
        let mut rng = thread_rng();
        self
          .replicas
          .choose(&mut rng)
          .cloned()
          .unwrap_or_else(|| self.primary.clone())
      }
      ReadSelection::LeastLatency => self
        .replicas
        .first()
        .cloned()
        .unwrap_or_else(|| self.primary.clone()),
    }
  }

  pub fn primary(&self) -> &Arc<P> {
    &self.primary
  }

  pub fn replicas(&self) -> &[Arc<P>] {
    &self.replicas
  }

  pub fn config(&self) -> &ReplicaConfig {
    &self.config
  }
}

impl ReplicaConfig {
  pub fn new(primary_url: impl Into<String>) -> Self {
    Self {
      primary_url: primary_url.into(),
      replica_urls: Vec::new(),
      read_selection: ReadSelection::default(),
      failover: true,
    }
  }

  pub fn with_replicas(mut self, urls: Vec<String>) -> Self {
    self.replica_urls = urls;
    self
  }

  pub fn with_read_selection(mut self, selection: ReadSelection) -> Self {
    self.read_selection = selection;
    self
  }

  pub fn with_failover(mut self, enabled: bool) -> Self {
    self.failover = enabled;
    self
  }
}
