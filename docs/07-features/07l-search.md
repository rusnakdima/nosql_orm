# Full-text search (coming soon)

> Status: **coming soon** — planned for 0.10.0. Not implemented in 0.9.0.

Full-text search across entity fields with ranking, highlighting and
language-aware tokenisation. Useful for filtering/searching data in apps
built on Forge.

---

## Planned API

```rust
// Repository-level
repo.search("title", "rust orm").await?;
repo.search_multi(&["title", "body"], "rust").await?;

// Query builder
Query::new().search("title", "rust").limit(20)
```

## Planned features

- Tokenisation with stop-word removal and stemming.
- Field-weighted ranking (title > body).
- Highlight snippets in results.
- Pluggable analyzers (default, simple, language-specific).
- Works across all providers (JSON, Mongo, SQL, Redis) with provider-specific
  optimisations (Mongo `$text`, Postgres `tsvector`, SQLite FTS5).

## Why coming soon

The core storage/query surface is stable; search needs a dedicated index
structure and analyzer pipeline that does not exist yet. Tracked as
`nosql_orm` roadmap item.

## See also

- [07f-indexes.md](07f-indexes.md) — index infrastructure (prerequisite)
- [07m-aggregation.md](07m-aggregation.md) — pipeline queries
