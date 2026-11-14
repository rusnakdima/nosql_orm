# Change Data Capture (coming soon)

> Status: **coming soon** — planned for 0.10.0. Not implemented in 0.9.0.

Change Data Capture (CDC) streams row-level changes (insert/update/delete)
from a collection so consumers can react in real time. Used for local↔cloud
sync, audit trails and event-driven workflows in Forge apps.

---

## Planned API

```rust
let stream = repo.watch().await?;
while let Some(event) = stream.next().await {
    match event {
        ChangeEvent::Insert { id, doc } => { ... }
        ChangeEvent::Update { id, before, after } => { ... }
        ChangeEvent::Delete { id } => { ... }
    }
}
```

## Planned features

- Insert / update / delete events with before/after snapshots.
- Resume tokens for reconnect-safe streams.
- Provider-native where possible (Mongo change streams, Postgres logical
  replication, polling fallback for JSON/SQLite).
- Filterable by collection and field predicates.

## Why coming soon

Requires a unified async stream abstraction over heterogeneous providers;
the JSON provider has no native change feed. Tracked as `nosql_orm` roadmap
item.

## See also

- [07g-events.md](07g-events.md) — in-process entity events (available now)
- [07p-subscription.md](07p-subscription.md) — real-time subscriptions (coming soon)
EOF