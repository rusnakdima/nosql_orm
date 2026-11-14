# nosql_orm Features

This directory contains detailed documentation for the nosql_orm feature set.

## Feature list (actual modules present in `src/`)

| Feature / module | File | Status |
|---|---|---|
| Macros | [07a-macros.md](07a-macros.md) | ✅ (`nosql_orm_derive`) |
| Validators | [07b-validators.md](07b-validators.md) | ✅ (`validators`) |
| Soft Delete | [07c-soft-delete.md](07c-soft-delete.md) | ✅ (`soft_delete`) |
| Timestamps | [07d-timestamps.md](07d-timestamps.md) | ✅ (`timestamps`) |
| Migrations | [07e-migrations.md](07e-migrations.md) | ✅ (`migrations`) — basic runner |
| Indexes | [07f-indexes.md](07f-indexes.md) | ✅ (`nosql_index`) |
| Events | [07g-events.md](07g-events.md) | ✅ (`events`) |
| Cascade | [07h-cascade.md](07h-cascade.md) | ✅ (`cascade`) |
| Lazy | [07i-lazy.md](07i-lazy.md) | ✅ opt-in (`lazy`) |
| Embedded | [07j-embedded.md](07j-embedded.md) | ✅ opt-in (`embedded`) |
| Inheritance | [07k-inheritance.md](07k-inheritance.md) | ✅ opt-in (`inheritance`) |
| Aggregation | [07m-aggregation.md](07m-aggregation.md) | ✅ (`aggregation`) |
| Cache | [07q-cache.md](07q-cache.md) | ✅ opt-in (`query_cache`) |
| SQL | [07r-sql.md](07r-sql.md) | ✅ (`sql`) |
| Logging | [07s-logging.md](07s-logging.md) | ✅ (`logging`) |

There are **no** `search`, `cdc`, `graphql` or `subscription` modules in this
crate, and no corresponding feature flags. Files `07l-search.md`,
`07n-cdc.md`, `07o-graphql.md` and `07p-subscription.md` are stale and
describe functionality that does not exist.

---

## Feature flags

```toml
[features]
default = ["json"]
json = []
mongo = ["dep:mongodb", "dep:futures-util", "dep:futures"]
redis = ["dep:redis"]
full = ["json", "mongo", "redis", "sql"]
sql = ["sql-postgres", "sql-sqlite", "sql-mysql"]
sql-postgres = ["dep:tokio-postgres", "dep:deadpool-postgres"]
sql-sqlite = ["dep:rusqlite"]
sql-mysql = ["dep:mysql_async"]
query_cache = []

# Opt-in extension modules (implemented, not wired into Storage/Repository)
embedded = []
inheritance = []
lazy = []
```

---

## Opt-in vs integrated

- **Integrated** (always available under their provider feature): soft delete,
  timestamps, validators, migrations, indexes, events, cascade, aggregation,
  SQL utilities, logging, query caching (under `query_cache`).
- **Opt-in extension modules** (feature-gated, self-contained, not wired into
  `Storage`/`Repository`/providers): `embedded`, `inheritance`, `lazy`.
- **Storage backends** (provider features): `json`, `mongo`, `redis`, `sql-*`.

---

## Quick overview

1. Start with [../01-introduction.md](../01-introduction.md)
2. [../02-entity.md](../02-entity.md) — Entity trait
3. [../03-provider.md](../03-provider.md) — Providers
4. [../04-repository.md](../04-repository.md) — Repository
5. [../05-query-builder.md](../05-query-builder.md) — Querying
6. [../06-relations.md](../06-relations.md) — Relations
7. This directory — advanced features
8. [../08-examples.md](../08-examples.md) — Examples
9. [../09-api-reference.md](../09-api-reference.md) — API reference