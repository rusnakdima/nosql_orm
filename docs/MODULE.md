# nosql_orm — Module Reference

Per-module deep dive into `nosql_orm`. The order mirrors the dependency flow: provider trait first, concrete providers next, repository / query on top, then cross-cutting features.

Conventions:

- LOC counts in parentheses next to file paths.
- Anchor references use `path:line` where helpful.

## 1. Provider Trait Layer

### `provider.rs` — `src/provider.rs`

The provider traits that everything else conforms to.

```text
ProviderConfig       → connection string + db name + option bag
Capability           → Transactions / Indexes / Aggregation / RawQueries
Storage              → insert / find_by_id / find_many / update / patch /
                       delete / delete_many / update_many / count / aggregate
                       / health_check / index ops (generic, Clone)
StorageBackend       → the object-safe subset of Storage (dyn-safe)
SchemaIntrospection  → list_collections / describe_collection / stats /
                       list_indexes / get_database_name / list_databases
AdminCommands        → execute_raw / create_collection / drop_collection /
                       rename_collection / get_server_version /
                       health_check_detailed
TransactionControl   → begin_transaction / commit_transaction /
                       rollback_transaction / is_transaction_active
```

Key items:

- `pub trait Storage: Send + Sync + Clone + 'static` (`src/provider.rs`) — the unified generic contract. Default impls provided for `exists`, `find_all`, `find_all_typed<T>`, `index_exists`, `aggregate` (defaults to `NotSupported`), `health_check` (defaults `true`) and `insert_many` (loops `insert`).
- `pub trait StorageBackend: Send + Sync` — the dyn-safe contract returned by `connect_backend`. A blanket impl forwards it to any provider that is also `SchemaIntrospection + AdminCommands`, and `impl Storage for Arc<dyn StorageBackend>` bridges it back to `Repository`.
- `ProviderCapabilities` — `none()` is the conservative default: a provider that says nothing supports nothing.
- `ProviderConfig` — connection string + optional database name + `options: HashMap<String, String>`.

### `admin_types.rs` (95) — `src/admin_types.rs`

DTOs returned by the `SchemaIntrospection` / `AdminCommands` traits.

| Type | Purpose |
| --- | --- |
| `CollectionMeta` | Lightweight summary of a collection (name, doc count, …). |
| `CollectionSchema` | Schema descriptor with field definitions. |
| `CollectionStats` | Doc count, size, index info. |
| `ConnectionHealth` | Health-check details (latency, status). |
| `FieldInfo` | Field name, nullable, type. |
| `IndexInfo` | Index name + key fields. |
| `PoolStats` | Pool saturation metrics. |
| `RawResult` | Wraps raw query results. |
| `TransactionId` | Opaque transaction handle. |

Re-exported from `provider.rs:8-13` and from `prelude`.

## 2. Concrete Providers — `providers/` (mod.rs, 132)

### `providers/json.rs` (760) — JSON file storage

The default backend (`json` feature, also included in `default = ["json"]` in `Cargo.toml:14`).

```text
JsonCacheConfig             → max_entries_per_collection + ttl_seconds
JsonProviderConfig          → base_dir + JsonCacheConfig (env-overridable)
JsonProvider (Cache + Disk) → Arc<RwLock<HashMap<collection, Vec<Value>>>>
                              backed by <base>/<collection>.json files
```

Highlights:

- Collection name validation (`validate_collection_name`, `src/providers/json.rs:60`) — rejects empty / too-long / path-traversal / dot-prefix names.
- Path resolution (`make_path`, `:79`).
- LRU-style eviction via `access_order: Arc<RwLock<HashMap<collection, VecDeque<id>>>>` (`:106`).
- Flush strategy — writes to disk on `insert` and `update`.
- Env override — `NOSQL_ORM_DATA_DIR` (`JsonProviderConfig::new`, `:42`).
- CRUD defaults: `insert` mints an id if missing; `update` supports `$inc` semantics (`src/providers/json.rs:280-303`).
- `SchemaIntrospection`, `AdminCommands`, `TransactionControl` implementations follow.

### `providers/redis.rs` (469) — Redis backend

`redis` feature. Uses `redis::aio::ConnectionManager` for async connection. Maps collections onto Redis hashes or key spaces. **Untested in unit tests** — needs a live Redis server (see `#[ignore]` plan in `library-quality-and-conductor-tests-plan.md` Phase B3).

### `providers/mongo/mod.rs` (528) — MongoDB backend

`mongo` feature. Sub-modules:

- `mongo/filter.rs` — `Filter` → MongoDB `Document`.
- `mongo/convert.rs` — `Value` ↔ BSON.
- `mongo/helpers.rs` — shared conversion helpers.
- `mongo/indexes.rs` — Mongo index management.

`MongoProvider` supports CRUD + indexes + aggregation. **Untested** — live-server dependency.

### `providers/sql/{postgres,mysql,sqlite}.rs` (681 / 700 / 744) — SQL backends

All three implement `Storage` plus `SchemaIntrospection` and `TransactionControl`. Each has its own dialect handling, prepared-statement cache, and JSON⇄row mapping in `providers/sql/common.rs`, `row.rs`, `utils.rs`.

| Backend | Feature | Notes |
| --- | --- | --- |
| `PostgresProvider` | `sql-postgres` | Uses `tokio-postgres` + `deadpool-postgres`. |
| `MySqlProvider` | `sql-mysql` | Uses `mysql_async`. |
| `SqliteProvider` | `sql-sqlite` | Bundled `rusqlite` — fully offline-capable. |

All three are **untested** in unit tests. The plan adds `#[ignore]` trait-conformance tests to each.

### `providers/sync.rs` — Provider sync helpers

`ConflictResolution` enum + `ProviderSync` + `SyncOptions` — used for replication scenarios.

### `providers/mod.rs` (132) — Registry + `FallbackProvider<P1, P2>`

`FallbackProvider` (`src/providers/mod.rs:8`) wraps two providers and:

- `find_with_fallback` (`:13`) — primary within timeout, fallback otherwise.
- `find_all_with_fallback` (`:50`) — same idea for batched finds.

Prints to `stderr` on primary errors / timeouts. Returns the first successful result. The `mod.rs` file also acts as the public surface, re-exporting each provider under its feature flag (`:67-87`).

## 3. Query Layer — `query/`

```text
filter.rs (251)            → Filter enum (18 variants) + matches() + from_json()
builder.rs (264)           → QueryBuilder
projection.rs (95)         → Projection
compiled.rs (269)          → compiled SQL-ish form
optimizer.rs (79)          → QueryOptimizer → OptimizedQuery
hints.rs (184)             → OptimizationHint, QueryAnalyzer, QueryHint
index_recommender.rs (85)  → IndexRecommendation, IndexRecommender, QueryStats
stats.rs (128)             → QueryCollectionStats, FilterPattern, QueryStatistics
profiling.rs (209)         → runtime query profiling
types.rs (70)              → Cursor, OrderBy, PaginatedResult, SortDirection
zero_copy.rs (171)         → DirectDeserializer, LazyValue, TypedArrayDeserializer,
                              ZeroCopyDeserialize
mod.rs (19)                → public re-exports
```

`Filter::matches` (`src/query/filter.rs:31`) is the workhorse — every provider reuses it for in-memory filtering. `Filter::from_json` (`:84`) parses a Mongo-style JSON descriptor into `Filter` for cross-backend query portability.

Macros (`src/macros.rs`, 98): `eq_filter`, `ne_filter`, `gt_filter`, `gte_filter`, `lt_filter`, `lte_filter`, `in_filter`, `not_filter`, `and_filter`, `or_filter`, `is_null_filter`, `is_not_null_filter`.

## 4. Repository Layer — `repository/`

```text
mod.rs (61)               → Repository<E, P>, QueryStream, SyncResult,
                             QueryTimeout, QueryStreamTrait
crud.rs (159)             → insert, update, save, save_from_value,
                             insert_many, update_many, upsert_many,
                             update_with_immutable, patch, delete_many
find.rs (125)             → find_by_id, get_by_id, find_all,
                             find_all_including_deleted, count, exists,
                             find_for_frontend, find_by_id_for_frontend,
                             *_with_timeout
delete.rs (90)            → delete, remove, soft_delete, restore
query.rs                  → filter-based queries
indexes.rs                → create_index, drop_index, list_indexes
relations.rs              → with_relations, eager loading
fast_path.rs              → hot-path optimization for known queries
```

Highlights:

- `Repository<E, P>` is generic over the entity and provider. Both bounds are required (`where E: Entity, P: Storage`).
- `save` (`crud.rs:53`) does upsert: inserts if `id` is `None`, otherwise updates.
- `delete` (`delete.rs:18`) consults relations and dispatches to `CascadeManager` when hard cascade is configured.
- `soft_delete` / `restore` (`delete.rs:57-86`) patch `deleted_at`.
- `find_for_frontend` (`:50`) requires `E: FrontendProjection`.
- `QueryStream<E, P>` (`mod.rs:46`) — streaming wrapper.

## 5. Entity Layer — `entity/`

```text
mod.rs (10)               → re-exports
inner.rs (100)            → Entity trait, EntityMeta, FrontendProjection
accessors.rs (123)        → Accessors, CachedAccessor, ComputedField,
                             EntityAccessors, AccessorsExecutor
mutators.rs (129)         → CastType, MutatorDef, Mutators,
                             EntityMutators, MutatorsExecutor
```

`Entity` (`src/entity/inner.rs:25`) requires `Serialize + DeserializeOwned + Debug + Send + Sync + 'static` and:

- `meta() -> EntityMeta`
- `get_id(&self) -> Option<String>`
- `set_id(&mut self, id: String)`
- Default hooks: `fields()`, `to_value`, `from_value`, `table_name`, `is_soft_deletable`, `indexes`, `sql_columns`.

`FrontendProjection` adds `frontend_excluded_fields()` and `filter_for_frontend()` for client payload filtering.

## 6. Relations Layer — `relations/`

```text
mod.rs (16)               → public re-exports
types.rs (187)            → RelationType, RelationDef, RelationValue,
                             TransformMapVia, WithLoaded, WithRelations
wrappers.rs               → ManyToMany, ManyToOne, OneToMany, OneToOne
helpers.rs                → apply_filter, filter_not_deleted, inject_collection
registry.rs               → register_collection_relations,
                             register_relations_for_entity,
                             get_collection_relations, get_relation_def, …
loader/mod.rs (one_to_many,
              many_to_one,
              many_to_many,
              recursive,
              nested,
              cascade)  → RelationLoader
traversal.rs              → RelationTraversal
```

`RelationDef` (`src/relations/types.rs:36`) is the canonical relation descriptor: type, target collection, source/target keys, and cascade behaviour. The four wrappers (`OneToOne`, `OneToMany`, `ManyToOne`, `ManyToMany`) provide ergonomic builders; `WithRelations` is implemented on the entity to declare its graph.

`register_collection_relations` (`src/relations/registry.rs`) is the central registry — providers and `CascadeManager` consult it via `get_collection_relations` (`src/cascade/mod.rs:101`).

`RelationLoader` (`src/relations/loader/mod.rs`) handles the different cardinalities — `one_to_many`, `many_to_one`, `many_to_many`, `recursive`, `nested`, and `cascade`.

## 7. Cascade Layer — `cascade/`

```text
mod.rs (199)              → CascadeManager + entry points
hard_delete.rs (90)       → hard_delete_cascade (entry),
                             process_hard_delete_cascade,
                             collect_cascade_hard_delete_one_to_many,
                             collect_cascade_single_side_hard
soft_delete.rs            → soft_delete_cascade
restore.rs                → restore cascade
helpers.rs                → cascade_value, insert_cascade_id,
                             CascadeEntityRef
```

`CascadeManager<P: Storage>` (`src/cascade/mod.rs:11`) wraps a provider and orchestrates cascading. Public methods:

- `soft_delete` / `restore` / `toggle_delete` (`:74, 76, 78`) — single-collection operations.
- `check_restrict` (`:24`) — true if the entity has at least one relation with restrict-on-delete policy.
- `hard_delete_cascade` (`hard_delete.rs:23`) — recursive hard delete with visited-id set.
- `should_cascade_*` / `should_restrict` — helpers delegating to `RelationDef`.

## 8. Pool Layer — removed

There is no `pool/` module. It was deleted in Phase 4: nothing in `Repository`,
`Storage` or the providers constructed a `JsonPool`/`MongoPool`, and the SQL
providers use their drivers' own pools (`deadpool-postgres`, `mysql_async`).
`PoolStats` survives in `admin_types.rs` because `ConnectionHealth` carries it.

## 9. SQL Layer — `sql/`

```text
mod.rs (14)               → re-exports
types.rs (535)            → SqlColumnType, SqlColumnDef, SqlPrimaryKey,
                             SqlIndexDef, SqlIndexType, SqlTableDef,
                             SqlDialect, SqlOnDelete
prepared.rs (167)         → PreparedStatement, BatchPreparedStatements,
                             PreparedStatementCache
query/mod.rs              → SqlQueryBuilder
query/builder.rs          → builder internals
query/ddl.rs              → CREATE / ALTER / DROP
query/dml.rs              → INSERT / UPDATE / DELETE
query/filter.rs           → Filter → SQL WHERE
```

The `sql/` module is the dialect-agnostic SQL toolkit; concrete dialects live in `providers/sql/{postgres,mysql,sqlite}.rs`.

## 10. Migrations — `migrations/`

```text
mod.rs (3)                → re-exports
migration.rs (111)        → Migration trait, MigrationMeta,
                             SqlMigration, JsonMigration
runner.rs (101)           → MigrationRunner
source.rs (67)            → source loader
```

`Migration<P: Storage>` (`src/migrations/migration.rs:17`) has `version()`, `name()`, `async up(&P)`, `async down(&P)`. Concrete `SqlMigration` and `JsonMigration` adapt raw SQL / JSON to the same contract.

## 11. Error Layer — `error.rs` (96)

`OrmError` (`src/error.rs:6`) — 22 variants:

```text
NotFound, Duplicate, Serialization, Io, Provider, NotSupported, Relation,
InvalidQuery, InvalidInput, Query, Connection, Transaction,
CascadeRestricted { entity, relation }, Mongo*, Redis*, Validation,
Internal, Timeout, PoolExhausted, QueryCancelled, ConstraintViolation,
Deadlock
```

(* = feature-gated). Plus helpers `map_err_connection` / `map_err_query` (`:78-86`) and the `OrmResult<T>` alias (`:96`).

## 12. Cross-cutting Features

| Module | Path | Purpose |
| --- | --- | --- |
| `logging/` | `src/logging/mod.rs` | Query logger, file-backed query logger, wrapper. |
| `cli/` | `src/cli/` | `commands`, `migrator`, `seeder` — optional CLI front-end. |
| `aggregation/` | `src/aggregation/` | `AggregationPipeline`, `GroupStage`, `MatchStage`, `ProjectStage`, `SortStage`, `LimitStage`, `SkipStage`, `accumulators`. |
| `constraints/` | `src/constraints/` | `ColumnConstraint`, `CheckConstraintDef`, `UniqueConstraintDef`, `IndexDef`, `IndexType`. |
| `events/` | `src/events/` | `EntityEvents`, `EntityEventListener`, `Event`, `EventType`. |
| `validators/` | `src/validators/` | `FieldValidator`, `EmailValidator`, `LengthValidator`, `PatternValidator`, `RangeValidator`, `ValidationError`, `ValidationResult`. |
| `model_helper.rs` | `src/model_helper.rs` | `ModelHelper` derive macro re-export, `WithValidation`, `ModelValidationError`. |
| `soft_delete.rs` | `src/soft_delete.rs` | `SoftDeletable` + `SoftDeleteExt`. |
| `timestamps.rs` | `src/timestamps.rs` | `Timestamps` trait + `apply_timestamps`. |
| `field_meta.rs` | `src/field_meta.rs` | `FieldMeta`, `FieldType`, `RelationFieldType`, `RelationMeta`, `EntityFields`, `ValidateMeta`, `ValidatorType`. |
| `macros.rs` | `src/macros.rs` | Filter helper constructors. |
| `utils.rs` | `src/utils.rs` | `compare_values`, `generate_id`, `get_document_id`. |

### Opt-in extension modules (feature-gated, unreferenced)

`cdc/`, `graphql/`, `subscription/`, `embedded/`, `inheritance/`, `lazy/` and
`search/` are compiled only under their own feature (`cdc`, `graphql`, …).
Nothing in `Storage`, `Repository` or the providers calls them. The
`observability/`, `security/`, `data_engineering/` and `transaction.rs` entries
that older revisions of this file listed do not exist on disk.

## 13. Plan Coverage Notes

This file is the canonical reference for the nosql_orm slice of `library-quality-and-conductor-tests-plan.md`. Concretely:

- **B3** adds unit tests for `sql/postgres.rs`, `sql/mysql.rs`, `sql/sqlite.rs`, `redis.rs`, `mongo/mod.rs`. The trait is `Storage`; the trait-conformance pattern is `insert → find_by_id → delete`. Live-server tests use `#[ignore]` with a "requires live server" comment.
- There is no trait rename: `Storage` is the generic trait and `StorageBackend` is its dyn-safe counterpart. Older documents that name `DatabaseProvider` are describing a trait that never existed.
- Library passes `cargo check`; run `cargo test -p nosql_orm --features json,sql-sqlite --lib` for the current unit-test count.