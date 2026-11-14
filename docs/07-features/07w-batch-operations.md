# Batch operations

> Status: **partially implemented** — `insert_many`, `update_many`,
> `upsert_many` and `delete_many` ship on `Repository` in 0.10.0.
> Per-row error reporting, partial success and provider-native bulk
> transports are **coming soon**.

Bulk insert/update/delete over a collection.

---

## Implemented API

```rust
// Repository (src/repository/crud.rs)
repo.insert_many(vec![a, b, c]).await?;          // -> count of inserted
repo.upsert_many(vec![a, b, c]).await?;          // save() per entity, needs E: Validate
repo.update_many(Some(filter), patch).await?;    // -> count matched/updated
repo.delete_many(Some(filter)).await?;           // -> count removed

// Storage trait, for raw documents
storage.insert_many("tasks", docs).await?;       // default: loops insert()
```

Also: `fast_path::exec_batch_by_ids` reads a list of ids in one call.

Semantics today — **all-or-nothing at the call site**:

- `insert_many` / `upsert_many` loop entity-by-entity and return the first
  error; a failure leaves earlier inserts in place (no rollback).
- `update_many` / `delete_many` are a single provider call and report the
  affected count.
- `Storage::insert_many` default is a plain loop; a provider may override it
  with a native bulk API.

Tested by `test_batch_operations` in `tests/test_sqlite_integration.rs`
(plus the Postgres/MySQL suites).

## Coming soon

- Per-row error reporting (which rows failed and why) instead of first-error.
- Partial-success mode (commit successes, report failures).
- Chunked execution with a configurable batch size.
- Provider-native bulk transports: Mongo `insertMany`, Postgres `COPY`,
  SQLite transaction batching.
- A `BatchResult` type aggregating counts + failures.

## See also

- [04-repository.md](../04-repository.md) — Repository
- [07v-transactions.md](07v-transactions.md) — transactions
