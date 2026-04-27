# Pub/Sub

Publish-subscribe functionality.

---

## Publisher

```rust
pub trait Publisher: Clone + Send + Sync {
    async fn publish(&self, topic: &str, message: Value) -> OrmResult<()>;
    async fn subscribe(&self, topic: &str) -> OrmResult<Subscription>;
}
```

## Subscription

```rust
pub struct Subscription {
    pub topic: String,
}
```

## SubscriptionManager

```rust
pub struct SubscriptionManager { ... }
```

## Topic

```rust
pub struct Topic {
    pub name: String,
    pub subscription_handler: Option<Box<dyn SubscriptionHandler>>,
}
```