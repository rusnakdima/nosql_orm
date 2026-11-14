# Offline-first sync (coming soon)

> Status: **coming soon** — planned for 0.11.0. Not implemented in 0.9.0.

Queue writes while offline and sync when connectivity returns, with
conflict resolution. Powers Forge apps that work without a network.

---

## Planned API

```rust
let sync = SyncEngine::new(local_storage, remote_storage);
sync.enqueue_write(collection, doc).await?;
sync.sync().await?; // drains queue, resolves conflicts
```

## Planned features

- Write-ahead queue persisted locally.
- Conflict resolution strategies (last-write-wins, merge, custom).
- Sync status events (pending, syncing, synced, conflict).
- Selective sync (per-collection, per-tenant).

## Why coming soon

Needs a queue store, conflict model and transport layer. Tracked as
`nosql_orm` roadmap item.

## See also

- [07n-cdc.md](07n-cdc.md) — change data capture (coming soon)
- [07u-multi-tenancy.md](07u-multi-tenancy.md) — multi-tenancy (coming soon)
