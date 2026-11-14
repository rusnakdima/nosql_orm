# Streaming results (coming soon)

> Status: **coming soon** — planned for 0.10.0. Not implemented in 0.9.0.

Stream large result sets lazily instead of loading everything into memory.
Essential for exports, reports and large collections.

---

## Planned API

```rust
let mut stream = repo.find_stream(filter).await?;
while let Some(row) = stream.next().await {
    process(row?);
}
```

## Planned features

- `find_stream`, `aggregate_stream` returning `async Stream<Item = Result<E>>`.
- Backpressure-aware (consumer controls pace).
- Provider-native cursors (Mongo cursor, SQL server-side cursor).
- JSON/SQLite fallback via chunked pagination.

## Why coming soon

Requires an async stream abstraction and cursor lifetime management across
providers. Tracked as `nosql_orm` roadmap item.

## See also

- [04-repository.md](../04-repository.md) — Repository
- [05-query-builder.md](../05-query-builder.md) — QueryBuilder
