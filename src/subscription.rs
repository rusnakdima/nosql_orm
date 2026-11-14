// Real-time subscriptions: push collection changes to subscribers.
//
// Wraps a `ChangeCaptureStorage` and dispatches change events to
// registered subscribers (callbacks or streams).

use crate::cdc::{ChangeEvent, ChangeOp};


use tokio::sync::broadcast;

#[derive(Clone)]
pub enum SubscriptionFilter {
  All,
  Collection(String),
  Operation(ChangeOp),
  CollectionAndOp(String, ChangeOp),
}

impl SubscriptionFilter {
  pub fn matches(&self, event: &ChangeEvent) -> bool {
    match self {
      SubscriptionFilter::All => true,
      SubscriptionFilter::Collection(c) => event.collection == *c,
      SubscriptionFilter::Operation(op) => {
        std::mem::discriminant(op) == std::mem::discriminant(&event.op)
      }
      SubscriptionFilter::CollectionAndOp(c, op) => {
        event.collection == *c
          && std::mem::discriminant(op) == std::mem::discriminant(&event.op)
      }
    }
  }
}

pub type ChangeSender = broadcast::Sender<ChangeEvent>;
pub type ChangeReceiver = broadcast::Receiver<ChangeEvent>;

#[derive(Clone)]
pub struct SubscriptionManager {
  sender: ChangeSender,
}

impl SubscriptionManager {
  pub fn new(capacity: usize) -> Self {
    let (sender, _) = broadcast::channel(capacity);
    Self { sender }
  }

  // Subscribe to change events matching the filter.
  pub fn subscribe(&self, filter: SubscriptionFilter) -> Subscription {
    let mut rx = self.sender.subscribe();
    let (tx, rx_out) = tokio::sync::mpsc::channel(256);
    tokio::spawn(async move {
      while let Ok(event) = rx.recv().await {
        if filter.matches(&event) {
          if tx.send(event).await.is_err() {
            break;
          }
        }
      }
    });
    Subscription { receiver: rx_out }
  }

  // Publish a change event to all subscribers.
  pub fn publish(&self, event: ChangeEvent) {
    let _ = self.sender.send(event);
  }

  // Number of active subscribers.
  pub fn subscriber_count(&self) -> usize {
    self.sender.receiver_count()
  }
}

pub struct Subscription {
  receiver: tokio::sync::mpsc::Receiver<ChangeEvent>,
}

impl Subscription {
  // Receive the next change event (async).
  pub async fn recv(&mut self) -> Option<ChangeEvent> {
    self.receiver.recv().await
  }

  // Try to receive without blocking.
  pub fn try_recv(&mut self) -> Option<ChangeEvent> {
    self.receiver.try_recv().ok()
  }
}

// Bridge between ChangeCaptureStorage and SubscriptionManager.
pub struct SubscribedStorage<P: crate::provider::Storage> {
  capture: crate::cdc::ChangeCaptureStorage<P>,
  manager: SubscriptionManager,
}

impl<P: crate::provider::Storage> SubscribedStorage<P> {
  pub fn new(inner: P, capacity: usize) -> Self {
    Self {
      capture: crate::cdc::ChangeCaptureStorage::new(inner),
      manager: SubscriptionManager::new(capacity),
    }
  }

  pub fn manager(&self) -> &SubscriptionManager {
    &self.manager
  }

  pub fn subscribe(&self, filter: SubscriptionFilter) -> Subscription {
    self.manager.subscribe(filter)
  }

  // Poll for new changes and publish them.
  pub async fn poll_changes(&self) {
    for event in self.capture.drain_changes() {
      self.manager.publish(event);
    }
  }
}

impl<P: crate::provider::Storage> std::ops::Deref for SubscribedStorage<P> {
  type Target = crate::cdc::ChangeCaptureStorage<P>;

  fn deref(&self) -> &Self::Target {
    &self.capture
  }
}