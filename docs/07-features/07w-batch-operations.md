# Batch operations

> Status: **implemented** — `insert_many`, `update_many`, `upsert_many` and
> `delete_many` ship on `Repository` in 0.10.0, and per-row error reporting
> (`BatchResult` / `insert_many_detailed` / `insert_many_chunked`) ships on
> `Storage`. Provider-native bulk transports are **coming soon**.

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

### Per-row error reporting (partial success)

```rust
use nosql_orm::batch::BatchResult;

let result: BatchResult = storage.insert_many_detailed("tasks", docs).await?;
// result.succeeded          -> rows written
// result.failed             -> Vec<(index_into_docs, OrmError)>
// result.is_success() / total() / summary()

// Chunked: batches of `batch_size`, failure indices stay global.
let result = storage.insert_many_chunked("tasks", docs, 500).await?;
```

Semantics:

- `insert_many` is still **first-error** (existing behaviour, unchanged):
  it stops at the first failure and returns `Err`.
- `insert_many_detailed` attempts **every** row: successes stay written,
  each failure keeps its index into the submitted `docs` and the reason.
  A provider-level failure (connection gone) still returns `Err`.
- `insert_many_chunked` runs `insert_many_detailed` in batches of
  `batch_size` (`InvalidInput` on `batch_size == 0`); the merged result
  reports indices into the original `docs` order. A chunk-level provider
  error stops the run and is returned as `Err` — rows already written are
  not undone.
- `update_many` / `delete_many` are a single provider call and report the
  affected count.
- `Storage::insert_many` default is a plain loop; a provider may override
  it with a native bulk API.

Tested by `tests/test_batch_errors.rs`, `test_batch_operations` in
`tests/test_sqlite_integration.rs` (plus the Postgres/MySQL suites).

## Coming soon

- Provider-native bulk transports: Mongo `insertMany`, Postgres `COPY`,
  SQLite transaction batching.
- Repository-level `insert_many` partial-success variant (today the
  `Storage`-level detailed API is the partial-success path).

## See also

- [04-repository.md](../04-repository.md) — Repository
- [07v-transactions.md](07v-transactions.md) — transactions
