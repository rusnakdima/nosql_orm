# Full-text search

> Status: **implemented (baseline)** — `Storage::search()` ships in 0.10.0.
> Ranking, stemming and provider-native indexes are **coming soon**.

Full-text search across entity fields. The baseline does case-insensitive
substring matching in memory so every provider answers the same way;
providers may override `search()` with a native index later.

---

## Implemented API

```rust
// Storage trait — every provider inherits this default
storage.search("tasks", &["title", "body"], "rust orm").await?;
```

```rust
// Default implementation (src/provider.rs)
async fn search(&self, collection: &str, fields: &[&str], query: &str)
  -> OrmResult<Vec<Value>>;
```

Behaviour:

- Loads the collection with `find_all`, then keeps documents where **any**
  of `fields` contains `query` (case-insensitive).
- Returns the matching documents as `Vec<Value>`; no ranking, no snippets.
- A field that is missing or not a string never matches (no panic).

Tested by `tests/test_new_features.rs::test_full_text_search`.

## Coming soon

- Tokenisation with stop-word removal and stemming.
- Field-weighted ranking (title > body) and highlight snippets.
- Pluggable analyzers (default, simple, language-specific).
- Provider-native indexes: Mongo `$text`, Postgres `tsvector`, SQLite FTS5.
  Providers override `search()` — the trait comment marks this as the
  extension point.

## See also

- [07f-indexes.md](07f-indexes.md) — index infrastructure (prerequisite)
- [07m-aggregation.md](07m-aggregation.md) — pipeline queries
