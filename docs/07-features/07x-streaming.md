# Streaming results

> Status: **implemented** — `Storage::find_stream()` and
> `Repository::find_stream()` ship in 0.10.0. Native provider cursors and
> `aggregate_stream` are **coming soon**.

Stream large result sets lazily instead of loading everything into memory.
Essential for exports, reports and large collections.

---

## Implemented API

```rust
use futures::StreamExt;

// Storage trait — every provider inherits the default
let mut stream = storage.find_stream("tasks", Some(filter));
while let Some(row) = stream.next().await {
  process(row?);
}

// Repository — typed, batched (default batch size 100)
let mut stream = repo.find_stream(Some(filter), skip, limit, sort_by, sort_asc).await?;
while let Some(entity) = stream.next().await {
  process(entity?);
}
```

- `Storage::find_stream` returns
  `Pin<Box<dyn Stream<Item = OrmResult<Value>> + Send>>`. The default
  collects once and streams the results; providers may override for native
  cursor support.
- `Repository::find_stream` returns `QueryStream<E, P>`, which fetches in
  batches of `batch_size` (default 100, set with `with_batch_size`).

Tested by `tests/test_new_features.rs::test_streaming`.

## Coming soon

- `aggregate_stream` for pipeline results.
- Provider-native cursors (Mongo cursor, SQL server-side cursor) — the
  trait already names this as the override point.
- Backpressure-aware chunking tuning on the default path.

## See also

- [04-repository.md](../04-repository.md) — Repository
- [05-query-builder.md](../05-query-builder.md) — QueryBuilder
