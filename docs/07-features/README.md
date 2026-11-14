# nosql_orm Features

This directory contains detailed documentation for the nosql_orm feature set.

## Feature list

### Implemented and integrated

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
| Aggregation | [07m-aggregation.md](07m-aggregation.md) | ✅ (`aggregation`) |
| Cache | [07q-cache.md](07q-cache.md) | ✅ opt-in (`query_cache`) |
| SQL | [07r-sql.md](07r-sql.md) | ✅ (`sql`) |
| Logging | [07s-logging.md](07s-logging.md) | ✅ (`logging`) |

### Implemented but not yet wired into Storage/Repository (opt-in)

| Feature / module | File | Status |
|---|---|---|
| Embedded | [07j-embedded.md](07j-embedded.md) | 🔧 opt-in (`embedded`) — self-contained, integration coming soon |
| Inheritance | [07k-inheritance.md](07k-inheritance.md) | 🔧 opt-in (`inheritance`) — self-contained, integration coming soon |
| Lazy | [07i-lazy.md](07i-lazy.md) | 🔧 opt-in (`lazy`) — self-contained, integration coming soon |

### Coming soon (not implemented)

| Feature / module | File | Status |
|---|---|---|
| Full-text search | [07l-search.md](07l-search.md) | 🚧 coming soon (0.10.0) |
| Change Data Capture | [07n-cdc.md](07n-cdc.md) | 🚧 coming soon (0.10.0) |
| Real-time subscriptions | [07p-subscription.md](07p-subscription.md) | 🚧 coming soon (0.10.0) |
| Vector / semantic search | [07t-vector-search.md](07t-vector-search.md) | 🚧 coming soon (0.11.0) |
| Multi-tenancy | [07u-multi-tenancy.md](07u-multi-tenancy.md) | 🚧 coming soon (0.10.0) |
| Transactions | [07v-transactions.md](07v-transactions.md) | 🚧 coming soon (0.10.0) |
| Batch operations | [07w-batch-operations.md](07w-batch-operations.md) | 🚧 coming soon (0.10.0) |
| Streaming results | [07x-streaming.md](07x-streaming.md) | 🚧 coming soon (0.10.0) |
| Offline-first sync | [07y-offline-sync.md](07y-offline-sync.md) | 🚧 coming soon (0.11.0) |
| Encryption at rest | [07z-encryption.md](07z-encryption.md) | 🚧 coming soon (0.11.0) |
| New SQL providers | [08a-new-sql-dbs.md](08a-new-sql-dbs.md) | 🚧 coming soon (0.11.0–0.12.0) |
| New NoSQL providers | [08b-new-nosql-dbs.md](08b-new-nosql-dbs.md) | 🚧 coming soon (0.11.0–0.12.0) |
| GraphQL provider | [08c-graphql-provider.md](08c-graphql-provider.md) | 🚧 coming soon (0.12.0) |
| Vector database providers | [08c-vector-dbs.md](08c-vector-dbs.md) | 🚧 coming soon (0.11.0) |

### Removed (not needed for this library)

| Feature | Reason |
|---|---|
| GraphQL | REST-style queries suffice for the app-builder use case; adds heavy dependency. Removed from docs. |
| (search/cdc/subscription files retained as "coming soon" — see above) | — |

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
  Integration into Storage/Repository is coming soon.
- **Storage backends** (provider features): `json`, `mongo`, `redis`, `sql-*`.
- **Coming soon**: full-text search, CDC, real-time subscriptions, vector
  search, multi-tenancy, transactions.

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
EOF
echo done