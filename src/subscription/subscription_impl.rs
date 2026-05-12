use crate::error::OrmResult;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

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
  pub id: String,
  pub topic: Topic,
  pub handler: Arc<dyn SubscriptionHandler>,
  pub options: SubscriptionOptions,
}

impl Clone for Subscription {
  fn clone(&self) -> Self {
    Subscription {
      id: self.id.clone(),
      topic: self.topic.clone(),
      handler: self.handler.clone(),
      options: self.options.clone(),
    }
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

#[derive(Clone)]
pub struct SubscriptionConfig {
  pub max_subscriptions: usize,
}

impl Default for SubscriptionConfig {
  fn default() -> Self {
    Self {
      max_subscriptions: 10000,
    }
  }
}

pub struct SubscriptionManager {
  subscriptions: std::collections::HashMap<String, std::collections::HashMap<String, Subscription>>,
  config: SubscriptionConfig,
  creation_order: std::collections::VecDeque<(String, String)>,
}

impl SubscriptionManager {
  pub fn new() -> Self {
    Self {
      subscriptions: std::collections::HashMap::new(),
      config: SubscriptionConfig::default(),
      creation_order: std::collections::VecDeque::new(),
    }
  }

  pub fn with_max_subscriptions(mut self, max: usize) -> Self {
    self.config.max_subscriptions = max;
    self
  }

  fn evict_if_needed(&mut self) {
    while self.creation_order.len() >= self.config.max_subscriptions {
      if let Some((topic_id, sub_id)) = self.creation_order.pop_front() {
        if let Some(subs) = self.subscriptions.get_mut(&topic_id) {
          subs.remove(&sub_id);
        }
        if self
          .subscriptions
          .get(&topic_id)
          .map_or(true, |s| s.is_empty())
        {
          self.subscriptions.remove(&topic_id);
        }
      }
    }
  }

  pub fn subscribe<S: SubscriptionHandler + 'static>(&mut self, topic: &str, handler: S) -> String {
    self.evict_if_needed();
    let id = Uuid::new_v4().to_string();
    let subscription = Subscription {
      id: id.clone(),
      topic: Topic::new(topic),
      handler: Arc::new(handler),
      options: SubscriptionOptions::default(),
    };
    self
      .subscriptions
      .entry(topic.to_string())
      .or_default()
      .insert(id.clone(), subscription);
    self
      .creation_order
      .push_back((topic.to_string(), id.clone()));
    id
  }

  pub fn unsubscribe(&mut self, topic: &str) {
    self.subscriptions.remove(topic);
  }

  pub fn unsubscribe_by_id(&mut self, id: &str) -> bool {
    for (topic, subs) in &mut self.subscriptions {
      if subs.remove(id).is_some() {
        self
          .creation_order
          .retain(|(t, s)| !(t == topic && s == id));
        return true;
      }
    }
    false
  }

  pub fn get_subscription(&self, id: &str) -> Option<&Subscription> {
    for subs in self.subscriptions.values() {
      if let Some(sub) = subs.get(id) {
        return Some(sub);
      }
    }
    None
  }

  pub fn subscription_count(&self) -> usize {
    self.creation_order.len()
  }
}

impl Default for SubscriptionManager {
  fn default() -> Self {
    Self::new()
  }
}
