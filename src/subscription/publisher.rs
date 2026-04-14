use crate::error::OrmResult;
use crate::subscription::subscription::SubscriptionMessage;
use tokio::sync::broadcast;

pub struct Publisher {
  sender: broadcast::Sender<SubscriptionMessage>,
}

impl Publisher {
  pub fn new(capacity: usize) -> Self {
    let (sender, _) = broadcast::channel(capacity);
    Self { sender }
  }

  pub fn publish(&self, message: SubscriptionMessage) -> OrmResult<()> {
    self
      .sender
      .send(message)
      .map_err(|_| crate::error::OrmError::Connection("Failed to publish message".to_string()))?;
    Ok(())
  }

  pub fn subscribe(&self) -> broadcast::Receiver<SubscriptionMessage> {
    self.sender.subscribe()
  }
}

impl Clone for Publisher {
  fn clone(&self) -> Self {
    Self {
      sender: self.sender.clone(),
    }
  }
}

#[cfg(feature = "redis")]
pub struct RedisPublisher {
  redis: crate::providers::RedisProvider,
}

#[cfg(feature = "redis")]
impl RedisPublisher {
  pub fn new(redis: crate::providers::RedisProvider) -> Self {
    Self { redis }
  }

  pub async fn publish(&self, topic: &str, message: SubscriptionMessage) -> OrmResult<()> {
    self
      .redis
      .publish(topic, &serde_json::to_value(&message)?)
      .await
  }
}
