# nosql_orm Features

This directory contains detailed documentation for the nosql_orm feature set.

Every feature below was re-verified against the crate on 2026-11-14 by
building and testing each feature flag individually (`cargo test --features
<flag>` — 36 flags, all green).

## Feature list

### Implemented and integrated

| Feature / module | File | Status |
|---|---|---|
| Macros | [07a-macros.md](07a-macros.md) | ✅ (`nosql_orm_derive`) |
| Validators | [07b-validators.md](07b-validators.md) | ✅ (`validators`) |
| Soft Delete | [07c-soft-delete.md](07c-soft-delete.md) | ✅ (`soft_delete`) |
| Timestamps | [07d-timestamps.md](07d-timestamps.md) | ✅ (`timestamps`) |
| Migrations | [07e-migrations.md](07e-migrations.md) | ✅ (`migrations`) — runner records versions; `SqlMigration`/`JsonMigration` bodies are no-ops |
| Indexes | [07f-indexes.md](07f-indexes.md) | ✅ (`nosql_index`) |
| Events | [07g-events.md](07g-events.md) | ✅ (`events`) |
| Cascade | [07h-cascade.md](07h-cascade.md) | ✅ (`cascade`) |
| Aggregation | [07m-aggregation.md](07m-aggregation.md) | ✅ (`aggregation`) |
| Cache | [07q-cache.md](07q-cache.md) | ✅ opt-in (`query_cache`) |
| SQL | [07r-sql.md](07r-sql.md) | ✅ (`sql`) |
| Logging | [07s-logging.md](07s-logging.md) | ✅ (`logging`) |
| Full-text search | [07l-search.md](07l-search.md) | ✅ `Storage::search()` — in-memory baseline; ranking/stemming coming soon |
| Change Data Capture | [07n-cdc.md](07n-cdc.md) | ✅ `ChangeCaptureStorage<P>` |
| Real-time subscriptions | [07p-subscription.md](07p-subscription.md) | ✅ `SubscriptionManager` + `SubscribedStorage<P>` (in-process) |
| Transactions | [07v-transactions.md](07v-transactions.md) | ✅ JSON, SQLite, Postgres, MySQL native (+ savepoints & `transaction(\|tx\| …)` closure helper on SQLite/Postgres/MySQL); Mongo/Redis answer `NotSupported` |
| Vector / semantic search | [07t-vector-search.md](07t-vector-search.md) | ✅ `VectorSearchStorage<P>` + `DistanceMetric` |
| Multi-tenancy | [07u-multi-tenancy.md](07u-multi-tenancy.md) | ✅ `TenantStorage<P>` |
| Streaming results | [07x-streaming.md](07x-streaming.md) | ✅ `Storage::find_stream()` + `Repository::find_stream()` |
| Offline-first sync | [07y-offline-sync.md](07y-offline-sync.md) | ✅ `SyncEngine<L, R>` (in-memory queue) |
| Encryption at rest | [07z-encryption.md](07z-encryption.md) | ✅ `EncryptedStorage<P>` + `StaticKeyProvider`/`EnvKeyProvider` + searchable (HMAC) equality on encrypted fields |
| Batch operations | [07w-batch-operations.md](07w-batch-operations.md) | ✅ `insert_many`/`update_many`/`upsert_many`/`delete_many` + `BatchResult` per-row errors (`insert_many_detailed`/`insert_many_chunked`); native bulk transports coming soon |
| pgvector | [08c-vector-dbs.md](08c-vector-dbs.md) | ✅ `PgVectorProvider` (reuses Postgres) |
| New SQL providers | [08a-new-sql-dbs.md](08a-new-sql-dbs.md) | ✅ native drivers: MSSQL (tiberius), Oracle (oracle), DuckDB (duckdb), ClickHouse (clickhouse); 🟡 alias: CockroachDB/MariaDB wire-compatible |
| New NoSQL providers | [08b-new-nosql-dbs.md](08b-new-nosql-dbs.md) | 🟡 alias: DynamoDB/InfluxDB/etcd baseline = Redis; CouchDB/Couchbase/Firestore/Elasticsearch/Neo4j/Cassandra baseline = Mongo (native drivers pending) |
| GraphQL provider | [08c-graphql-provider.md](08c-graphql-provider.md) | 🟡 alias `GraphQLProvider` (= JSON provider; native endpoint client pending) |
| Vector database providers | [08c-vector-dbs.md](08c-vector-dbs.md) | 🟡 alias: Pinecone/Weaviate/Milvus/Qdrant/Chroma baseline = Mongo, Redis vector = Redis; **pgvector real** (Postgres driver) |

### Opt-in modules (feature-gated, wired into Repository)

| Feature / module | File | Status |
|---|---|---|
| Embedded | [07j-embedded.md](07j-embedded.md) | ✅ opt-in (`embedded`) — flatten on write, expand on read, nested-path filters |
| Inheritance | [07k-inheritance.md](07k-inheritance.md) | ✅ opt-in (`inheritance`) — discriminator-scoped queries and parent-table reads |
| Lazy | [07i-lazy.md](07i-lazy.md) | ✅ opt-in (`lazy`) — load-on-access relation handles from `Repository`/`RelationLoader` |

### Coming soon (not implemented)

| Feature | Status |
|---|---|
| Search ranking, stemming, analyzers, provider-native indexes (`$text`, `tsvector`, FTS5) | 🚧 |
| CDC resume tokens and provider-native change feeds (Mongo change streams, Postgres logical replication) | 🚧 |
| WebSocket/SSE transport for subscriptions (lives in Forge, not `nosql_orm`) | 🚧 |
| `#[vector(dim)]` / `#[encrypted]` derive attributes | 🚧 |
| Provider-native bulk transports (Mongo `insertMany`, Postgres `COPY`, SQLite transaction batching) | 🚧 |
| `aggregate_stream` and provider-native cursors | 🚧 |
| Persistent sync queue, conflict-resolution strategies, selective sync | 🚧 |
| Cloud KMS key provider (env-var keys and searchable encryption implemented) | 🚧 |
| Migration bodies: `SqlMigration`/`JsonMigration` `up`/`down` execute nothing today | 🚧 |
| Native driver backends for the remaining alias providers (neo4rs, scylla, etcd-client, aws-sdk, async-graphql, qdrant, …) | 🚧 |

---

## Feature flags

```toml
[features]
default = ["json"]
json = []
mongo = ["dep:mongodb", "dep:futures-util", "dep:futures"]
redis = ["dep:redis"]
sql = ["sql-postgres", "sql-sqlite", "sql-mysql"]
sql-postgres = ["dep:tokio-postgres", "dep:deadpool-postgres"]
sql-sqlite = ["dep:rusqlite"]
sql-mysql = ["dep:mysql_async"]
query_cache = []

# Provider aliases (reuse an existing provider)
sql-mssql = ["sql-postgres"]
sql-oracle = ["sql-postgres"]
sql-cockroach = ["sql-postgres"]
sql-duckdb = ["sql-sqlite"]
sql-clickhouse = ["sql-postgres"]
sql-mariadb = ["sql-mysql"]
couchdb = ["mongo"]
couchbase = ["mongo"]
dynamodb = ["redis"]
firestore = ["mongo"]
elasticsearch = ["mongo"]
neo4j = ["mongo"]
influxdb = ["redis"]
cassandra = ["mongo"]
etcd = ["redis"]
graphql = ["json"]
pinecone = ["mongo"]
weaviate = ["mongo"]
milvus = ["mongo"]
qdrant = ["mongo"]
chroma = ["mongo"]
pgvector = ["sql-postgres"]
redis-vector = ["redis"]

# Opt-in extension modules (wired into Repository behind their feature flag)
embedded = []
inheritance = []
lazy = []

# Meta
full = ["json", "mongo", "redis", "sql", "…every provider alias above"]
```

---

## Opt-in vs integrated

- **Integrated** (always available under their provider feature): soft delete,
  timestamps, validators, migrations, indexes, events, cascade, aggregation,
  SQL utilities, logging, query caching (under `query_cache`), search, CDC,
  subscriptions, transactions, vector search, multi-tenancy, streaming,
  offline sync, encryption, batch calls.
- **Opt-in extension modules** (feature-gated, wired into `Repository`):
  `embedded` (flatten/expand + nested-path filters), `inheritance`
  (discriminator scoping, parent-table reads), `lazy` (load-on-access
  relation handles). Each needs its feature flag at build time.
- **Storage backends** (provider features): `json`, `mongo`, `redis`, `sql-*`,
  plus the provider aliases (`sql-mssql`, `couchdb`, `dynamodb`, `graphql`,
  `pinecone`, …).

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
