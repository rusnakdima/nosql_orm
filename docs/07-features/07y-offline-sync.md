# Offline-first sync

> Status: **implemented (core)** — `SyncEngine<L, R>` ships in 0.10.0.
> Persistent queues, custom merge strategies and selective sync are
> **coming soon**.

Queue writes while offline and sync when connectivity returns, with
automatic retry. Powers Forge apps that work without a network.

---

## Implemented API

```rust
use nosql_orm::sync::{SyncEngine, SyncOp};

let sync = SyncEngine::new(local_storage, remote_storage);

// writes local immediately, queues the entry for the remote
sync.enqueue_write("tasks", SyncOp::Insert, doc).await?;
sync.enqueue_write("tasks", SyncOp::Update, updated).await?;
sync.enqueue_write("tasks", SyncOp::Delete, doc).await?;

let synced = sync.sync().await?;   // drains the queue, returns successes
sync.pending_count();              // entries still queued
```

- `SyncStatus`: `Pending`, `Syncing`, `Synced`, `Conflict(String)`,
  `Failed(String)`; `SyncEntry` carries collection, op, doc and status.
- `sync()` applies each entry to the remote. Failures are **re-queued** so
  nothing is lost; only successes are counted in the returned `usize`.
- `SyncEngine<L: Storage, R: Storage>` works over any two providers — e.g.
  JSON local ↔ MongoDB remote.

## Coming soon

- Write-ahead queue persisted locally (today the queue is in memory).
- Conflict-resolution strategies (last-write-wins, merge, custom) — the
  `Conflict(String)` status exists but no resolver runs yet.
- Sync status events and selective sync (per-collection, per-tenant).

## See also

- [07n-cdc.md](07n-cdc.md) — change data capture (event source)
- [07u-multi-tenancy.md](07u-multi-tenancy.md) — multi-tenancy
