# Real-time subscriptions

> Status: **implemented (in-process)** — `SubscriptionManager` +
> `SubscribedStorage<P>` ship in 0.10.0. A transport layer (WebSocket/SSE)
> lives in Forge and is **coming soon**.

Subscribers register a filter and receive `ChangeEvent`s published while the
app runs, so Forge apps show live data without polling.

---

## Implemented API

```rust
use nosql_orm::subscription::{
  SubscribedStorage, SubscriptionFilter, SubscriptionManager,
};

// one wrapper = storage + manager
let store = SubscribedStorage::new(inner, /* capacity */ 64);
let mut sub = store.subscribe(SubscriptionFilter::Collection("tasks".into()));

store.insert("tasks", doc).await?;
store.poll_changes().await?;            // drain CDC into the manager

if let Some(ev) = sub.recv().await { /* ... */ }   // async
let ev = sub.try_recv();                            // non-blocking
```

Also available standalone: `SubscriptionManager::new(capacity)`,
`.subscribe(filter)`, `.publish(event)`, `.subscriber_count()`.
`SubscriptionFilter::matches(&ChangeEvent)` decides delivery.

Tested by `tests/test_new_features.rs::test_subscription_receives_events`.

## Coming soon

- Reconnect with last-seen cursor (needs CDC resume tokens).
- Backpressure handling beyond the bounded channel.
- WebSocket/SSE adapters — these belong in Forge, not in `nosql_orm`.

## See also

- [07n-cdc.md](07n-cdc.md) — change data capture (the event source)
- [07g-events.md](07g-events.md) — in-process entity events (available now)
