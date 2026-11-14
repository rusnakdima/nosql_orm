# Real-time subscriptions (coming soon)

> Status: **coming soon** — planned for 0.10.0. Not implemented in 0.9.0.

Real-time subscriptions push collection changes to clients over a transport
(WebSocket / SSE) so Forge apps can show live data without polling.

---

## Planned API

```rust
let sub = repo.subscribe(Filter::all()).await?;
// in a handler
sub.on_change(|event| { ... });
```

## Planned features

- Filterable subscriptions per collection.
- Reconnect with last-seen cursor.
- Backpressure handling.
- Transport-agnostic core (WebSocket/SSE adapters in Forge).

## Why coming soon

Depends on CDC (07n) and a transport layer that lives in Forge, not in
`nosql_orm`. Tracked as `nosql_orm` roadmap item.

## See also

- [07n-cdc.md](07n-cdc.md) — change data capture (coming soon)
- [07g-events.md](07g-events.md) — in-process entity events (available now)
EOF