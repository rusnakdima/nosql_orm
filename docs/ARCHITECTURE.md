# nosql_orm — Architecture Overview

**Crate:** `nosql_orm` v0.9.0
**Role:** the only data access layer of the workspace. `forge` and the kernel
apps read and write through it; nothing else opens a collection directly.

## 1. Mission

`nosql_orm` is a TypeORM-inspired ORM that delivers one ergonomic API across
heterogeneous storage backends. A caller writes `Repository<Entity, P>` and the
provider behind `P` decides whether the bytes land in a JSON file, MongoDB,
Redis or a SQL database.

Primary responsibilities:

- A backend-agnostic CRUD + query interface: `Storage`, `Repository<E, P>`,
  `Filter`, `QueryBuilder`.
- A typed entity system (`Entity`, `EntityMeta`, derive macros) with optional
  validation (`Validate`, `validators`).
- Cross-cutting concerns: relations, cascade delete, soft delete, indexes
  (`nosql_index`), migrations, query logging, aggregation, caching.

## 2. The two provider traits

There is no `DatabaseProvider` trait. The crate has two provider traits.

| Trait | File | Purpose |
|---|---|---|
| `Storage` | `src/provider.rs` | Generic contract. `Repository<E, P>` is parameterised on `P: Storage`, so the compiler sees the concrete provider type and monomorphises calls. |
| `StorageBackend` | `src/provider.rs` | Dyn-safe subset used when the provider is chosen at runtime. `connect_backend` returns `Arc<dyn StorageBackend>`. |

`Storage` is `Send + Sync + Clone + 'static` and covers writes, reads, indexes,
`aggregate`, `health_check` and the capability query.

`StorageBackend` is object-safe. Two impls connect the traits:

- a blanket `impl<P: Storage + SchemaIntrospection + AdminCommands> StorageBackend for P`
  forwards every method to the concrete provider;
- `impl Storage for Arc<dyn StorageBackend>` bridges a runtime backend back to
  the generic `Repository`.

Backend difference is expressed as data: `Capability` names the optional
operations (`Transactions`, `Indexes`, `Aggregation`, `RawQueries`), and
`ProviderCapabilities::none()` means "supports nothing". Operations outside a
backend's contract return `OrmError::NotSupported`.

Companion traits, implemented by the same provider types:

| Trait | Role |
|---|---|
| `SchemaIntrospection` | `list_collections`, `describe_collection`, `get_collection_stats`, `list_indexes`, database names. |
| `AdminCommands` | `execute_raw`, `create_collection`, `drop_collection`, `rename_collection`, `get_server_version`, `health_check_detailed`. |
| `TransactionControl` | `begin_transaction`, `commit_transaction`, `rollback_transaction`, `is_transaction_active` over a `TransactionId`. |

## 3. Crate map

```text
src/
├── lib.rs              module declarations, root re-exports, prelude
├── provider.rs         Storage, StorageBackend, Capability, ProviderConfig,
│                       SchemaIntrospection, AdminCommands, TransactionControl
├── admin_types.rs      CollectionMeta, CollectionSchema, CollectionStats,
│                       ConnectionHealth, IndexInfo, RawResult, TransactionId
├── repository/         Repository<E, P>, RelationRepository, indexes, fast path,
│                       query, delete, crud, find, relations, mod
├── query/              Filter, QueryBuilder, Projection, optimizer, hints,
│                       index recommender, stats, profiling, types, zero_copy
├── entity/             Entity, EntityMeta, accessors, mutators, FrontendProjection
├── relations/          RelationDef, loaders, registry, traversal, wrappers
├── cascade/            cascade delete and restore across relations
├── nosql_index/        index definitions and IndexManager
├── migrations/         Migration, SqlMigration, JsonMigration, MigrationRunner, source
├── sql/                dialect-agnostic SQL types, prepared statements, query builder
├── validators/         field validators, ValidationError, ValidationResult
├── logging/            query logger, file query logger, db query logger, wrapper
├── aggregation/        MongoDB-style pipeline stages
├── cache/              QueryCache (feature = "query_cache")
├── constraints/        ColumnDef, IndexDef, ColumnConstraint (used by Schema)
├── schema/             Schema, SchemaManager, table prefixes
├── events/             EntityEvents, EntityEventListener
├── field_meta.rs       field-level metadata
├── id/                 id strategies and generator
├── macros.rs           eq_filter, and_filter, … constructors + filter!/filters!
├── model_helper.rs     ModelHelper, WithValidation
├── soft_delete.rs      SoftDeletable, SoftDeleteExt
├── timestamps.rs       Timestamps, apply_timestamps
├── utils.rs            compare_values, generate_id, get_document_id, DocumentExt
└── providers/
    ├── json.rs         JSON file provider              (feature = "json")
    ├── json_tree.rs    one-file-per-document provider  (feature = "json")
    ├── mongo/          MongoDB provider                (feature = "mongo")
    ├── redis.rs        Redis provider                  (feature = "redis")
    ├── sql/            Postgres, MySQL, SQLite         (feature = "sql-*")
    ├── backend.rs      connect_backend, BackendKind, ConnectionParts
    ├── sync.rs         provider-to-provider sync
    └── variant.rs      provider hand-off (json + mongo)
```

### Feature-gated extension modules

Compiled only when their feature is enabled. Self-contained implementations,
not wired into `Storage`, `Repository` or any provider.

| Module | Feature | Contents |
|---|---|---|
| `embedded/` | `embedded` | `Embedded`, `EmbeddedMeta`, `Embedder`, `EmbedExt` |
| `inheritance/` | `inheritance` | `Discriminator`, single-table inheritance |
| `lazy/` | `lazy` | `Lazy`, `LazyLoader`, `LazyMany`, `LazyRelation` |

`cache/` (query cache) is gated on `query_cache` and is reachable from the
prelude when enabled.

## 4. Public API surface

- **Traits:** `Storage`, `StorageBackend`, `SchemaIntrospection`,
  `AdminCommands`, `TransactionControl`, `Entity`, `SoftDeletable`,
  `WithRelations`, `Migration`, `Validate`, `FieldValidator`,
  `FrontendProjection`, `Timestamps`, `Embedded`, `Inheritance`,
  `Discriminated`.
- **Providers:** `JsonProvider`, `JsonTreeProvider`, `MongoProvider`,
  `RedisProvider`, `PostgresProvider`, `MySqlProvider`, `SqliteProvider`,
  `FallbackProvider<P1, P2>`.
- **Repository:** `Repository<E, P>` with CRUD, find, queries, index
  management, streaming; `RelationRepository<E, P>`; `CascadeManager<P>`.
- **Query:** `Filter`, `QueryBuilder`, `Projection`, `OrderBy`,
  `SortDirection`, `Cursor`, `PaginatedResult`.
- **DTOs:** `CollectionMeta`, `CollectionSchema`, `CollectionStats`,
  `ConnectionHealth`, `IndexInfo`, `PoolStats`, `RawResult`, `TransactionId`.
- **Errors:** `OrmError`, `OrmResult<T>`.

## 5. Integration

```rust
use nosql_orm::prelude::{JsonProvider, Repository, Storage};

let provider = JsonProvider::new("./data").await?;
let users: Repository<User, _> = Repository::new(provider);
let user = users.save(user).await?;
```

`forge` uses `providers::json::JsonProvider` for the content tree,
`providers::json_tree::JsonTreeProvider` for the 3NF JSON DB,
`providers::backend::connect_backend` for runtime-selected backends and
`providers::sync::ProviderSync` for local-to-cloud push.

## 6. Feature flags

| Feature | Effect |
|---|---|
| `json` (default) | `JsonProvider`, `JsonTreeProvider` |
| `mongo` | `MongoProvider` |
| `redis` | `RedisProvider` |
| `sql-postgres`, `sql-mysql`, `sql-sqlite` | one SQL provider each |
| `sql` | all three SQL providers |
| `full` | `json`, `mongo`, `redis`, `sql` |
| `query_cache` | `cache::QueryCache` |
| `embedded`, `inheritance`, `lazy` | one extension module each |

There are no provider stub features: a backend either has a provider
implementation or no feature.

## 7. Tests

- `cargo test -p nosql_orm --features json,sql-sqlite --lib` runs the unit
  tests; module tests live in `src/<module>/tests/mod.rs`.
- Integration tests in `tests/` cover the JSON provider, SQLite provider,
  migrations, relations, cascade, soft delete, timestamps, indexes, validators
  and the query builder. Postgres, MySQL and Redis suites need a live server.
- `examples/` holds runnable examples for JSON, MongoDB and the SQL providers.

## 8. Known limitations

| Limitation | Detail |
|---|---|
| Backend-specific operations | Index management is `NotSupported` on Redis and a no-op/empty on JSON providers; `execute_raw` is `NotSupported` outside the SQL providers; aggregation is implemented only for JSON and Mongo. |
| SQL transactions | PostgreSQL and MySQL run transactions over connections checked out of their own pools; SQLite holds one connection. |
| Migrations | `SqlMigration` / `JsonMigration` `up`/`down` are no-ops; `MigrationRunner` records applied versions in a `_migrations` collection but does not execute the migration body. `SqlFileLoader` parses `--- DOWN ---`-separated SQL files but the resulting `SqlMigration` still does not run them. |
| Extension modules | `embedded`, `inheritance`, `lazy` are implemented but not wired into `Storage`, `Repository` or providers. |
| SQL provider duplication | `providers/sql/{postgres,mysql}.rs` share helpers in `sql/common.rs` but are not one generic implementation. |
| Lazy loading | `LazyRelation::get` fetches by `local_id` from its repository and ignores the relation descriptor. |

## 9. Further reading

- `src/lib.rs` — module declarations, re-exports, prelude.
- `src/provider.rs` — the two provider traits and the capability model.
- `README.md` — usage overview and the backend feature matrix.
- `docs/MODULE.md` — per-module breakdown.
- `docs/07-features/` — one page per opt-in capability.