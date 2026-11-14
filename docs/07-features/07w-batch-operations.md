# Batch operations (coming soon)

> Status: **coming soon** — planned for 0.10.0. Not implemented in 0.9.0.

Bulk insert/update/delete in a single round-trip, with per-row error
reporting and partial-success semantics.

---

## Planned API

```rust
repo.insert_many(&[a, b, c]).await?;
repo.update_many(filter, patch).await?;
repo.delete_many(filter).await?;
```

## Planned features

- `insert_many`, `update_many`, `delete_many`, `upsert_many`.
- Chunked execution with configurable batch size.
- Per-row error reporting (which rows failed and why).
- Partial-success mode (commit successes, report failures).
- Provider-native bulk APIs where available (Mongo `insertMany`,
  Postgres `COPY`, SQLite transaction batching).

## Why coming soon

Needs a unified batch handle and error-aggregation type across providers.
Tracked as `nosql_orm` roadmap item.

## See also

- [04-repository.md](../04-repository.md) — Repository
- [07v-transactions.md](07v-transactions.md) — transactions (coming soon)
