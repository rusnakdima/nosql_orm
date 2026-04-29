use super::strategy::IdStrategy;
use std::sync::Arc;

pub struct IdGenerator {
  strategy: Arc<dyn IdStrategy>,
}

impl IdGenerator {
  pub fn new(strategy: impl IdStrategy + 'static) -> Self {
    Self {
      strategy: Arc::new(strategy),
    }
  }

  pub fn default_instance() -> Self {
    Self::new(super::strategy::UuidStrategy)
  }

  pub fn with_uuid() -> Self {
    Self::new(super::strategy::UuidStrategy)
  }

  pub async fn generate(&self) -> crate::error::OrmResult<String> {
    self.strategy.generate().await
  }

  pub fn is_valid(&self, id: &str) -> bool {
    self.strategy.is_valid(id)
  }
}

impl Clone for IdGenerator {
  fn clone(&self) -> Self {
    Self {
      strategy: self.strategy.clone(),
    }
  }
}

impl Default for IdGenerator {
  fn default() -> Self {
    Self::default_instance()
  }
}
