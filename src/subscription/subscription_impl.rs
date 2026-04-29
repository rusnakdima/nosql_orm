use crate::error::OrmResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Topic {
  pub name: String,
  pub filter: Option<String>,
}

impl Topic {
  pub fn new(name: &str) -> Self {
    Self {
      name: name.to_string(),
      filter: None,
    }
  }

  pub fn with_filter(mut self, filter: &str) -> Self {
    self.filter = Some(filter.to_string());
    self
  }
}

#[async_trait::async_trait]
pub trait SubscriptionHandler: Send + Sync {
  async fn handle(&self, message: SubscriptionMessage) -> OrmResult<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionMessage {
  pub topic: String,
  pub payload: serde_json::Value,
  pub timestamp: chrono::DateTime<chrono::Utc>,
  pub message_id: String,
  pub correlation_id: Option<String>,
}

impl SubscriptionMessage {
  pub fn new(topic: &str, payload: serde_json::Value) -> Self {
    Self {
      topic: topic.to_string(),
      payload,
      timestamp: chrono::Utc::now(),
      message_id: uuid::Uuid::new_v4().to_string(),
      correlation_id: None,
    }
  }

  pub fn with_correlation(mut self, correlation_id: &str) -> Self {
    self.correlation_id = Some(correlation_id.to_string());
    self
  }
}

pub struct Subscription {
  pub topic: Topic,
  pub handler: Box<dyn SubscriptionHandler>,
  pub options: SubscriptionOptions,
}

impl Clone for Subscription {
  fn clone(&self) -> Self {
    panic!("Subscription handler cannot be cloned")
  }
}

#[derive(Debug, Clone)]
pub struct SubscriptionOptions {
  pub auto_ack: bool,
  pub max_retries: u32,
  pub retry_delay_ms: u64,
}

impl Default for SubscriptionOptions {
  fn default() -> Self {
    Self {
      auto_ack: true,
      max_retries: 3,
      retry_delay_ms: 1000,
    }
  }
}

pub struct SubscriptionManager {
  subscriptions: std::collections::HashMap<String, Vec<Subscription>>,
}

impl SubscriptionManager {
  pub fn new() -> Self {
    Self {
      subscriptions: std::collections::HashMap::new(),
    }
  }

  pub fn subscribe<S: SubscriptionHandler + 'static>(&mut self, topic: &str, handler: S) {
    let subscription = Subscription {
      topic: Topic::new(topic),
      handler: Box::new(handler),
      options: SubscriptionOptions::default(),
    };
    self
      .subscriptions
      .entry(topic.to_string())
      .or_default()
      .push(subscription);
  }

  pub async fn unsubscribe(&mut self, topic: &str) {
    self.subscriptions.remove(topic);
  }
}

impl Default for SubscriptionManager {
  fn default() -> Self {
    Self::new()
  }
}
