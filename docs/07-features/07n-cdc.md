# Change Data Capture

> Status: **implemented** — `ChangeCaptureStorage<P>` ships in 0.10.0.
> Resume tokens and provider-native change feeds are **coming soon**.

CDC records row-level changes (insert/update/delete) as they pass through a
storage wrapper, so consumers can react in real time. Used for local↔cloud
sync, audit trails and event-driven workflows in Forge apps.

---

## Implemented API

```rust
use nosql_orm::cdc::{ChangeCaptureStorage, ChangeEvent, ChangeOp};

let cdc = ChangeCaptureStorage::new(json_provider);
cdc.insert("tasks", doc).await?;

// one-shot consumers
let events: Vec<ChangeEvent> = cdc.drain_changes();   // takes them
let peek: Vec<ChangeEvent> = cdc.peek_changes();      // leaves them

// streaming consumer
let mut stream = cdc.changes_stream();
while let Some(ev) = stream.next().await { /* ... */ }

// bound the log
let cdc = ChangeCaptureStorage::new(inner).with_max_log_size(10_000);
```

`ChangeEvent` carries `op: ChangeOp` (`Insert` / `Update` / `Delete`),
`collection`, `id`, `before: Option<Value>`, `after: Option<Value>` and
`timestamp`. Constructors `ChangeEvent::insert/update/delete` fill the
before/after halves.

Tested by `tests/test_new_features.rs::test_cdc_captures_changes`.

## Coming soon

- Resume tokens for reconnect-safe streams.
- Provider-native feeds (Mongo change streams, Postgres logical
  replication, polling fallback stays for JSON/SQLite).
- Filterable by collection and field predicates.

## See also

- [07g-events.md](07g-events.md) — in-process entity events (available now)
- [07p-subscription.md](07p-subscription.md) — real-time subscriptions
