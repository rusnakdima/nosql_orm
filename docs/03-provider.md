# Providers

`nosql_orm` exposes one storage contract (`Storage`) implemented by every
backend. This document covers the traits, each provider and how to choose one.

---

## Provider traits

### `Storage` (generic, `Clone`)

```rust
#[async_trait]
pub trait Storage: Send + Sync + Clone + 'static {
    async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value>;
    async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>>;
    async fn find_many(&self, collection, filter, skip, limit, sort_by, sort_asc) -> OrmResult<Vec<Value>>;
    async fn update(&self, collection, id, doc) -> OrmResult<Value>;
    async fn patch(&self, collection, id, patch) -> OrmResult<Value>;
    async fn delete(&self, collection, id) -> OrmResult<bool>;
    async fn delete_many(&self, collection, filter) -> OrmResult<usize>;
    async fn update_many(&self, collection, filter, updates) -> OrmResult<usize>;
    async fn count(&self, collection, filter) -> OrmResult<u64>;
    async fn exists(&self, collection, id) -> OrmResult<bool>;       // default
    async fn find_all(&self, collection) -> OrmResult<Vec<Value>>;  // default
    async fn find_all_typed<T>(&self, collection) -> OrmResult<Vec<T>>; // default
    async fn create_index(&self, collection, index) -> OrmResult<()>;
    async fn drop_index(&self, collection, index_name) -> OrmResult<()>;
    async fn list_indexes(&self, collection) -> OrmResult<Vec<IndexInfo>>;
    async fn index_exists(&self, collection, index_name) -> OrmResult<bool>; // default
    async fn aggregate(&self, collection, pipeline) -> OrmResult<Vec<Value>>; // default NotSupported
    async fn health_check(&self) -> OrmResult<bool>;                  // default true
    async fn insert_many(&self, collection, docs) -> OrmResult<usize>; // default loop
    fn capabilities(&self) -> ProviderCapabilities;                    // default none
    fn supports(&self, capability) -> bool;                           // default
}
```

### `StorageBackend` (dyn-safe)

Object-safe subset used when the backend is chosen at runtime:
`connect_backend(kind, url)` returns `Arc<dyn StorageBackend>`. A blanket impl
forwards `Storage`+`SchemaIntrospection`+`AdminCommands` providers to it, and
`impl Storage for Arc<dyn StorageBackend>` bridges it back to `Repository`.

### Companion traits

- `SchemaIntrospection`: `list_collections`, `describe_collection`,
  `get_collection_stats`, `list_indexes`, `get_database_name`, `list_databases`.
- `AdminCommands`: `execute_raw`, `create_collection`, `drop_collection`,
  `rename_collection`, `get_server_version`, `health_check_detailed`.
- `TransactionControl`: `begin_transaction`, `commit_transaction`,
  `rollback_transaction`, `is_transaction_active` over `TransactionId`.

### Capability model

```rust
pub enum Capability { Transactions, Indexes, Aggregation, RawQueries }
pub struct ProviderCapabilities { ... }  // none() / all() / with(cap)
```

---

## JSON provider (`json` feature, default)

```rust
pub struct JsonProvider { ... }          // collection → Vec<Value> in memory + .json file
pub struct JsonProviderConfig { base_dir, cache_config }
pub struct JsonCacheConfig { max_entries_per_collection, ttl_seconds }
```

Constructors:

```rust
JsonProvider::new(base_dir: impl AsRef<Path>) -> OrmResult<Self>
JsonProvider::with_config(config: JsonProviderConfig) -> OrmResult<Self>
```

- Stores one `<collection>.json` file per collection under `base_dir`.
- `JsonProviderConfig::new` honours the `NOSQL_ORM_DATA_DIR` env var (prepended
  to `base_dir`).
- Collection names are validated (`validate_collection_name`): non-empty,
  ≤255 chars, no `..`, `/`, `\`, no leading dot.
- `capabilities()`: `Aggregation` only. Index ops are no-ops / empty.
- Implements `SchemaIntrospection`, `AdminCommands`, `TransactionControl`.

### JSON tree provider (`json` feature)

```rust
pub struct JsonTreeProvider;
JsonTreeProvider::new(base_dir: impl AsRef<Path>) -> OrmResult<Self>
```

One file per document (used by the Forge content tree). `capabilities()`: none.

---

## MongoDB provider (`mongo` feature)

```rust
pub struct MongoProvider { ... }
MongoProvider::connect(uri, db_name) -> OrmResult<Self>
MongoProvider::from_config(&ProviderConfig) -> OrmResult<Self>
```

- Sub-modules: `mongo/filter` (Filter → BSON), `mongo/convert`,
  `mongo/helpers`, `mongo/indexes`.
- Extra methods: `create_mongo_index`, `drop_mongo_index`.
- `capabilities()`: `Indexes`, `Aggregation`.

---

## Redis provider (`redis` feature)

```rust
pub struct RedisProvider { ... }
RedisProvider::new(connection_string: &str) -> OrmResult<Self>
RedisProvider::with_prefix(self, prefix: &str) -> Self   // default "nosql_orm:"
```

- Stores documents as JSON strings keyed `<prefix><collection>:<id>`, with a
  set of ids per collection.
- `capabilities()`: none. Indexes `NotSupported`.
- Extra method: `publish(channel, message)` (pub/sub primitive).
- `TransactionControl` is implemented.

---

## SQL providers (`sql-*` features)

```rust
pub struct SqliteProvider { ... }
SqliteProvider::connect(path) -> OrmResult<Self>
SqliteProvider::from_config(&ProviderConfig) -> OrmResult<Self>

pub struct PostgresProvider { ... }
PostgresProvider::connect(uri) -> OrmResult<Self>

pub struct MySqlProvider { ... }
MySqlProvider::connect(uri) -> OrmResult<Self>
```

- All implement `Storage`, `SchemaIntrospection`, `TransactionControl`.
- Extra helpers: `dialect()`, `query_builder()`, `build_select(...)`.
- `capabilities()`: Postgres/MySQL → `Indexes`, `RawQueries`; SQLite →
  `Indexes`, `Transactions`, `RawQueries`.
- `execute_raw` runs arbitrary SQL and returns `RawResult { affected_rows, rows }`.

### Runtime selection

```rust
pub enum BackendKind { Json, Mongo, Redis, Postgres, MySql, Sqlite }
pub async fn connect_backend(kind: BackendKind, url: &str) -> OrmResult<Arc<dyn StorageBackend>>
pub async fn connect_by_name(name: &str, url: &str) -> OrmResult<Arc<dyn StorageBackend>>
```

`dialect_from_connection(url)` infers a `SqlDialect` from the URL/path.

---

## Fallback provider

```rust
pub struct FallbackProvider<P1: Storage, P2: Storage> {
    pub primary: P1,
    pub fallback: P2,
}
```

Methods: `find_with_fallback(collection, filter, timeout)` and
`find_all_with_fallback(collection, filter, skip, limit, timeout)`. Primary is
tried within the timeout; on error/timeout/empty it falls back to the
secondary.

---

## Provider sync

```rust
pub struct ProviderSync<P1, P2>;
pub enum ConflictResolution { ... }
pub struct SyncOptions { ... }
```

Used for provider-to-provider replication. See `src/providers/sync.rs`.

---

## Choosing a provider

| Need | Pick |
|---|---|
| Zero config / tests / local dev | `JsonProvider` |
| One file per document (Forge content) | `JsonTreeProvider` |
| Production document store | `MongoProvider` |
| Cache / pub-sub | `RedisProvider` |
| Relational + raw SQL | `PostgresProvider` / `MySqlProvider` / `SqliteProvider` |
| Runtime-selected backend | `connect_backend` + `Arc<dyn StorageBackend>` |

---

## Next steps

- [04-repository.md](04-repository.md) — Repository CRUD
- [07-features/](07-features/) — Feature modules