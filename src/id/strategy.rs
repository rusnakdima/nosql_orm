use crate::error::OrmResult;
use async_trait::async_trait;

#[async_trait]
pub trait IdStrategy: Send + Sync {
  async fn generate(&self) -> OrmResult<String>;

  fn is_valid(&self, id: &str) -> bool {
    !id.is_empty()
  }
}

pub struct UuidStrategy;

#[async_trait]
impl IdStrategy for UuidStrategy {
  async fn generate(&self) -> OrmResult<String> {
    Ok(uuid::Uuid::new_v4().to_string())
  }
}

pub struct AutoIncrementStrategy {
  counter: std::sync::atomic::AtomicU64,
  prefix: Option<String>,
}

impl AutoIncrementStrategy {
  pub fn new() -> Self {
    Self {
      counter: std::sync::atomic::AtomicU64::new(1),
      prefix: None,
    }
  }

  pub fn with_prefix(mut self, prefix: &str) -> Self {
    self.prefix = Some(prefix.to_string());
    self
  }

  pub fn next_value(&self) -> u64 {
    self
      .counter
      .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
  }
}

#[async_trait]
impl IdStrategy for AutoIncrementStrategy {
  async fn generate(&self) -> OrmResult<String> {
    let id = self.next_value();
    Ok(match &self.prefix {
      Some(p) => format!("{}{}", p, id),
      None => id.to_string(),
    })
  }

  fn is_valid(&self, id: &str) -> bool {
    if let Some(prefix) = &self.prefix {
      id.starts_with(prefix) && id[prefix.len()..].parse::<u64>().is_ok()
    } else {
      id.parse::<u64>().is_ok()
    }
  }
}

pub struct CustomStrategy {
  generator: Box<dyn Fn() -> String + Send + Sync>,
}

impl CustomStrategy {
  pub fn new<F>(generator: F) -> Self
  where
    F: Fn() -> String + Send + Sync + 'static,
  {
    Self {
      generator: Box::new(generator),
    }
  }
}

#[async_trait]
impl IdStrategy for CustomStrategy {
  async fn generate(&self) -> OrmResult<String> {
    Ok((self.generator)())
  }
}

pub struct NanoidStrategy {
  size: usize,
  alphabet: String,
}

impl NanoidStrategy {
  pub fn new(size: usize) -> Self {
    Self {
      size,
      alphabet: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".to_string(),
    }
  }

  pub fn with_alphabet(mut self, alphabet: &str) -> Self {
    self.alphabet = alphabet.to_string();
    self
  }
}

#[async_trait]
impl IdStrategy for NanoidStrategy {
  async fn generate(&self) -> OrmResult<String> {
    let len = self.alphabet.len();
    let mut id = String::with_capacity(self.size);

    for _ in 0..self.size {
      let idx = rand::random::<usize>() % len;
      id.push(self.alphabet.chars().nth(idx).unwrap_or('a'));
    }
    Ok(id)
  }
}
