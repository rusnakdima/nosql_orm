# nosql_orm — Architecture Overview

**Crate:** `nosql_orm` v0.9.0
**Location:** `/mnt/external/Projects/nosql_orm/`
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
- Cross-cutting concerns the engine relies on: relations, cascade delete,
  soft delete, indexes (`nosql_index`), migrations, query logging.

## 2. The two provider traits

There is no `DatabaseProvider` trait. The crate has two provider traits, and
they exist for different reasons.

| Trait | File | Purpose |
| --- | --- | --- |
| `Storage` | `src/provider.rs` | The generic contract. `Repository<E, P>` is parameterised on `P: Storage`, so the compiler sees the concrete provider type and monomorphises the calls. |
| `StorageBackend` | `src/provider.rs` | The dyn-safe subset used when the provider is chosen at runtime. `connect_backend` returns `Arc<dyn StorageBackend>`. |

`Storage` is `Send + Sync + Clone + 'static` and covers:

- writes: `insert`, `insert_many`, `update`, `patch`, `update_many`, `delete`,
  `delete_many`;
- reads: `find_by_id`, `find_many`, `find_all`, `find_all_typed`, `count`,
  `exists`;
- indexes: `create_index`, `drop_index`, `list_indexes`, `index_exists`;
- `aggregate`, `health_check` and the capability query.

`StorageBackend` is object-safe: it declares the same methods without generic
parameters and without `Clone`. Two impls connect the traits:

- a blanket `impl<P: Storage + SchemaIntrospection + AdminCommands> StorageBackend for P`
  forwards every method to the concrete provider;
- `impl Storage for Arc<dyn StorageBackend>` bridges a runtime backend back to
  the generic `Repository`, so both entry points use one API.

Backend difference is expressed as data, not as a silent success: `Capability`
names the optional operations (`Transactions`, `Indexes`, `Aggregation`,
`RawQueries`), and `ProviderCapabilities::none()` means "supports nothing".
Operations outside a backend's contract return `OrmError::NotSupported`.

Companion traits, implemented by the same provider types:

| Trait | Role |
| --- | --- |
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
├── repository/         Repository<E, P>, RelationRepository, indexes, fast path
├── query/              Filter, QueryBuilder, Projection, optimizer, hints,
│                       query statistics
├── entity/             Entity, EntityMeta, accessors, mutators
├── relations/          RelationDef, loaders, registry, traversal, wrappers
├── cascade/            cascade delete and restore across relations
├── nosql_index/        index definitions and IndexManager
├── migrations/         Migration, SqlMigration, JsonMigration, MigrationRunner
├── sql/                dialect-agnostic SQL types, prepared statements, builder
├── validators/         field validators, ValidationError, ValidationResult
├── logging/            query logger, file query logger, provider wrapper
├── cli/                command parsing, migration and seeder commands
├── aggregation/        MongoDB-style pipeline stages
├── constraints/        ColumnDef, IndexDef, ColumnConstraint (used by Schema)
├── schema/             Schema, SchemaManager, table prefixes
├── events/             EntityEvents, EntityEventListener
├── field_meta.rs       field-level metadata
├── id/                 id strategies and generator
├── macros.rs           eq_filter, and_filter, … constructors
├── model_helper.rs     ModelHelper, WithValidation, ModelValidationError
├── soft_delete.rs      SoftDeletable, SoftDeleteExt
├── timestamps.rs       Timestamps, apply_timestamps
├── utils.rs            compare_values, generate_id, get_document_id
└── providers/
    ├── json.rs         JSON file provider              (feature = "json")
    ├── json_tree.rs    one-file-per-document provider  (feature = "json")
    ├── mongo/          MongoDB provider                (feature = "mongo")
    ├── redis.rs        Redis provider                  (feature = "redis")
    ├── sql/            Postgres, MySQL, SQLite         (feature = "sql-*")
    ├── backend.rs      connect_backend, BackendKind, ConnectionParts
    ├── sync.rs         provider-to-provider sync
    └── variant.rs      provider hand-off (json + mongo together)
```

### Feature-gated extension modules

These modules are compiled only when their feature is enabled. They are
self-contained implementations and are **not** wired into `Storage`,
`Repository` or any provider; nothing in the crate calls them.

| Module | Feature | Contents |
| --- | --- | --- |
| `graphql/` | `graphql` | `GraphQLSchema`, `SchemaBuilder`, resolvers, type mapping |
| `cdc/` | `cdc` | change capture, change stream, audit log, sync |
| `subscription/` | `subscription` | `Publisher`, `Subscription`, `Topic` |
| `inheritance/` | `inheritance` | `Discriminator`, single-table inheritance |
| `search/` | `search` | `FullTextSearch`, `FullTextIndex`, scoring |
| `embedded/` | `embedded` | `Embedded`, `Embedder`, `EmbedExt` |
| `lazy/` | `lazy` | `Lazy`, `LazyLoader`, `LazyMany` |

`cache/` (the query cache) is gated on `query_cache` and is reachable from the
prelude when enabled. The prelude re-exports the gated modules only under the
same `cfg`, so `use nosql_orm::prelude::*` stays valid in every feature set.

## 4. Public API surface

What callers import and depend on:

- **Traits:** `Storage`, `StorageBackend`, `SchemaIntrospection`,
  `AdminCommands`, `TransactionControl`, `Entity`, `SoftDeletable`,
  `WithRelations`, `Migration`.
- **Providers:** `JsonProvider`, `JsonTreeProvider`, `MongoProvider`,
  `RedisProvider`, `PostgresProvider`, `MySqlProvider`, `SqliteProvider`,
  `FallbackProvider<P1, P2>`.
- **Repository:** `Repository<E, P>` with CRUD (`insert`, `save`, `update`,
  `patch`, `delete`, `delete_many`, `update_many`, `upsert_many`), find
  (`find_by_id`, `find_all`, `count`, `exists`, `find_for_frontend`,
  `find_all_with_timeout`), queries (`query`, `query_stream`, `with_relations`)
  and index management (`create_index`, `drop_index`, `list_indexes`).
- **Query:** `Filter` (`Eq`, `Ne`, `Gt`, `Gte`, `Lt`, `Lte`, `In`, `NotIn`,
  `ArrayContains`, `Contains`, `StartsWith`, `EndsWith`, `Like`, `IsNull`,
  `IsNotNull`, `Between`, `And`, `Or`, `Not`), `QueryBuilder`, `Projection`,
  `OrderBy`, `SortDirection`.
- **DTOs:** `CollectionMeta`, `CollectionSchema`, `CollectionStats`,
  `ConnectionHealth`, `IndexInfo`, `PoolStats`, `RawResult`, `TransactionId`.
- **Errors:** `OrmError`, `OrmResult<T>`.

Type names are unique across the public API. Two renames removed the last
collisions: the JSON provider's document-cache settings are
`providers::json::JsonCacheConfig` (not `CacheConfig`), and the query layer's
per-collection timing record is `query::QueryCollectionStats` (not
`CollectionStats`, which stays the provider DTO in `admin_types`).

## 5. Integration

The workspace integrates through the prelude or through explicit paths:

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
| --- | --- |
| `json` (default) | `JsonProvider`, `JsonTreeProvider` |
| `mongo` | `MongoProvider` |
| `redis` | `RedisProvider` |
| `sql-postgres`, `sql-mysql`, `sql-sqlite` | one SQL provider each |
| `sql` | all three SQL providers |
| `full` | `json`, `mongo`, `redis`, `sql` |
| `query_cache` | `cache::QueryCache` |
| `graphql`, `cdc`, `subscription`, `inheritance`, `search`, `embedded`, `lazy` | one extension module each (see §3) |

There are no provider stub features: a backend either has a provider
implementation or no feature.

## 7. Tests

- `cargo test -p nosql_orm --features json,sql-sqlite --lib` runs the unit
  tests; module tests live in `src/<module>/tests/mod.rs`.
- Integration tests in `tests/` cover the JSON provider, the SQLite provider,
  migrations, relations, cascade, soft delete, timestamps, indexes, validators
  and the query builder. The Postgres, MySQL and Redis suites are declared with
  `required-features` and need a live server.
- `examples/` holds runnable examples for JSON, MongoDB and the SQL providers.

## 8. Known limitations

| Limitation | Detail |
| --- | --- |
| Backend-specific operations | Index management is `NotSupported` on the JSON, JsonTree and Redis providers; `execute_raw` is `NotSupported` outside the SQL providers. |
| SQL transactions | PostgreSQL and MySQL run a transaction over connections checked out of their own pools; SQLite holds one connection. |
| Extension modules | `graphql`, `cdc`, `subscription`, `inheritance`, `search`, `embedded` and `lazy` are implemented but unreferenced: they are opt-in features, not integrated behaviour. |
| SQL provider duplication | `providers/sql/{postgres,mysql}.rs` share helpers in `sql/common.rs` but not one generic implementation. |

## 9. Further reading

- `src/lib.rs` — module declarations, re-exports, prelude.
- `src/provider.rs` — the two provider traits and the capability model.
- `README.md` — usage overview and the backend feature matrix.
- `docs/MODULE.md` — per-module breakdown.
- `docs/07-features/` — one page per opt-in capability.
