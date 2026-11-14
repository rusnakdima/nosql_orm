# nosql_orm — Module Reference

Per-module deep dive. Order mirrors dependency flow: provider trait first,
concrete providers, repository / query, then cross-cutting features.

LOC counts in parentheses.

## 1. Provider trait layer

### `provider.rs` (523)

```text
ProviderConfig       → connection string + db name + option bag
Capability           → Transactions / Indexes / Aggregation / RawQueries
Storage              → insert / find_by_id / find_many / update / patch /
                       delete / delete_many / update_many / count / aggregate
StorageBackend       → object-safe subset of Storage
SchemaIntrospection  → list_collections / describe_collection / stats /
                       list_indexes / get_database_name / list_databases
AdminCommands        → execute_raw / create_collection / drop_collection /
                       rename_collection / get_server_version /
                       health_check_detailed
TransactionControl   → begin_transaction / commit_transaction /
                       rollback_transaction / is_transaction_active
```

- `Storage: Send + Sync + Clone + 'static`. Default impls for `exists`,
  `find_all`, `find_all_typed<T>`, `index_exists`, `aggregate`
  (defaults to `NotSupported`), `health_check` (defaults `true`) and
  `insert_many` (loops `insert`).
- `StorageBackend: Send + Sync`. Blanket impl forwards it to any provider that
  is also `SchemaIntrospection + AdminCommands`; `impl Storage for
  Arc<dyn StorageBackend>` bridges it back to `Repository`.
- `ProviderCapabilities`: `none()` is the conservative default.
- `ProviderConfig`: connection string + optional database name +
  `options: HashMap<String, String>`.

### `admin_types.rs` (95)

DTOs: `CollectionMeta`, `CollectionSchema`, `CollectionStats`,
`ConnectionHealth`, `FieldInfo`, `IndexInfo`, `PoolStats`, `RawResult`,
`TransactionId`. Re-exported from `provider.rs` and the prelude.

## 2. Concrete providers — `providers/`

### `providers/mod.rs` (128)

`FallbackProvider<P1, P2>` with `find_with_fallback` and
`find_all_with_fallback`. Re-exports each provider under its feature flag.

### `providers/json.rs` (741) — JSON file storage (feature `json`, default)

`JsonCacheConfig`, `JsonProviderConfig` (env-overridable via
`NOSQL_ORM_DATA_DIR`), `JsonProvider`. Collection name validation
(`validate_collection_name`). LRU-style eviction via `access_order`. Flush on
insert/update. `capabilities()`: `Aggregation` only.

### `providers/json_tree.rs` (571) — one file per document (feature `json`)

`JsonTreeProvider`. `capabilities()`: none.

### `providers/mongo/` (feature `mongo`)

`MongoProvider` plus `filter`, `convert`, `helpers`, `indexes` submodules.
`capabilities()`: `Indexes`, `Aggregation`. Extra: `create_mongo_index`,
`drop_mongo_index`.

### `providers/redis.rs` (508) (feature `redis`)

`RedisProvider` with `with_prefix`. `capabilities()`: none. Indexes
`NotSupported`. Extra: `publish`.

### `providers/sql/` (feature `sql-*`)

`SqliteProvider`, `PostgresProvider`, `MySqlProvider`, plus `common`,
`executor`, `row`, `utils`. `capabilities()`: Postgres/MySQL → `Indexes`,
`RawQueries`; SQLite → `Indexes`, `Transactions`, `RawQueries`. Extra:
`dialect()`, `query_builder()`, `build_select(...)`.

### `providers/backend.rs` (227)

`BackendKind`, `connect_backend(kind, url)`, `connect_by_name(name, url)`,
`ConnectionParts`, `dialect_from_connection`.

### `providers/sync.rs` (107)

`ConflictResolution`, `ProviderSync`, `SyncOptions`.

### `providers/variant.rs` (172)

Provider hand-off (json + mongo together).

## 3. Query layer — `query/`

```text
filter.rs (312)            → Filter enum (19 variants) + matches() + from_json()
builder.rs (225)           → QueryBuilder
projection.rs (95)         → Projection
optimizer.rs (80)          → QueryOptimizer → OptimizedQuery
hints.rs (184)             → OptimizationHint, QueryAnalyzer, QueryHint
index_recommender.rs (88)  → IndexRecommendation, IndexRecommender, QueryStats
stats.rs (139)             → QueryCollectionStats, FilterPattern, QueryStatistics
profiling.rs (177)         → runtime query profiling
types.rs (70)              → Cursor, OrderBy, PaginatedResult, SortDirection
zero_copy.rs (143)         → DirectDeserializer, LazyValue, TypedArrayDeserializer,
                              ZeroCopyDeserialize
mod.rs (19)                → public re-exports
```

`Filter::matches` is reused by every provider for in-memory filtering.
`Filter::from_json` parses Mongo-style JSON descriptors.
Macros (`src/macros.rs`, 99): `eq_filter`, `ne_filter`, … plus `filter!` and
`filters!`.

## 4. Repository layer — `repository/`

```text
mod.rs (342)              → Repository<E, P>, QueryStream, SyncResult,
                             QueryTimeout, QueryStreamTrait
crud.rs (184)             → insert, update, save, save_from_value,
                             insert_many, update_many, upsert_many,
                             update_with_immutable, patch, delete_many
find.rs (149)             → find_by_id, get_by_id, find_all,
                             find_all_including_deleted, count, exists,
                             find_for_frontend, find_by_id_for_frontend,
                             *_with_timeout
delete.rs (83)            → delete, remove, soft_delete, restore
query.rs (203)            → filter-based queries (RepositoryQuery)
indexes.rs (32)           → create_index, drop_index, list_indexes
relations.rs (236)        → with_relations, eager loading
fast_path.rs (103)        → hot-path optimization
```

- `Repository<E, P>`: `where E: Entity, P: Storage`.
- `save` does upsert.
- `QueryStream<E, P>` streams in batches (default 100).
- `RelationRepository<E, P>` requires `E: WithRelations`.

## 5. Entity layer — `entity/`

```text
mod.rs (10)               → re-exports
inner.rs (125)            → Entity trait, EntityMeta, FrontendProjection
accessors.rs (119)        → Accessors, CachedAccessor, ComputedField,
                             EntityAccessors, AccessorsExecutor
mutators.rs (129)         → CastType, MutatorDef, Mutators,
                             EntityMutators, MutatorsExecutor
```

`Entity` (`src/entity/inner.rs`) requires
`Serialize + DeserializeOwned + Debug + Send + Sync + 'static` and provides
`meta()`, `get_id()`, `set_id()`, plus default hooks.

## 6. Relations layer — `relations/`

```text
mod.rs (19)               → public re-exports
types.rs (206)            → RelationType, RelationDef, RelationValue,
                             TransformMapVia, WithLoaded, WithRelations
wrappers.rs (118)         → ManyToMany, ManyToOne, OneToMany, OneToOne
helpers.rs (39)           → apply_filter, filter_not_deleted, inject_collection
registry.rs (120)         → register_collection_relations,
                             register_relations_for_entity, get_collection_relations,
                             get_relation_def, …
loader/ (mod + one_to_many, many_to_one, many_to_many, recursive,
         nested, cascade)  → RelationLoader
traversal.rs (100)        → RelationTraversal
```

## 7. Cascade layer — `cascade/`

```text
mod.rs (270)              → CascadeManager + entry points
hard_delete.rs (148)      → hard_delete_cascade, process_hard_delete_cascade, …
soft_delete.rs (165)      → soft_delete_cascade
restore.rs (121)          → restore cascade
helpers.rs (39)           → cascade_value, insert_cascade_id, CascadeEntityRef
```

`CascadeManager<P: Storage>` wraps a provider and orchestrates cascading.

## 8. SQL layer — `sql/`

```text
mod.rs (11)               → re-exports
types.rs (523)            → SqlColumnType, SqlColumnDef, SqlPrimaryKey,
                             SqlIndexDef, SqlIndexType, SqlTableDef,
                             SqlDialect, SqlOnDelete
prepared.rs (168)         → PreparedStatement, BatchPreparedStatements,
                             PreparedStatementCache
query/mod.rs (6)          → SqlQueryBuilder
query/builder.rs (113)    → builder internals
query/ddl.rs (9)          → CREATE / ALTER / DROP
query/dml.rs (118)        → INSERT / UPDATE / DELETE
query/filter.rs (381)     → Filter → SQL WHERE
introspection.rs (49)     → TableIntrospector, build_select
```

## 9. Migrations — `migrations/`

```text
mod.rs (3)                → re-exports
migration.rs (103)        → Migration trait, MigrationMeta, SqlMigration, JsonMigration
runner.rs (100)           → MigrationRunner
source.rs (67)            → MigrationSource, MigrationLoader, SqlFileLoader
```

`Migration<P: Storage>`: `version()`, `name()`, `up(&P)`, `down(&P)`.
`MigrationRunner`: `run_all_pending`, `rollback`, `status`.
Note: `SqlMigration`/`JsonMigration` `up`/`down` are no-ops; the runner
records applied versions but does not execute the migration body.

## 10. Error layer — `error.rs` (94)

`OrmError` with 22 variants (see 09-api-reference). Helpers
`map_err_connection`, `map_err_query`; alias `OrmResult<T>`.

## 11. Cross-cutting features

| Module | Path | Purpose |
|---|---|---|
| `logging/` | `src/logging/` | Query logger, file-backed query logger, db query logger, wrapper. |
| `aggregation/` | `src/aggregation/` | `AggregationPipeline`, `GroupStage`, `MatchStage`, `ProjectStage`, `SortStage`, `LimitStage`, `SkipStage`, accumulators. |
| `constraints/` | `src/constraints/` | `ColumnConstraint`, `CheckConstraintDef`, `UniqueConstraintDef`, `IndexDef`, `IndexType`, `ColumnDef`, `ColumnType`. |
| `events/` | `src/events/` | `EntityEvents`, `EntityEventListener`, `Event`, `EventType`, `InsertEvent`, `UpdateEvent`, `DeleteEvent`, `QueryEvent`. |
| `validators/` | `src/validators/` | `FieldValidator`, `EmailValidator`, `LengthValidator`, `PatternValidator`, `RangeValidator`, `EnumValidator`, `CompositeValidator`, `EntityValidator`, `ValidationError`, `ValidationResult`. |
| `model_helper.rs` | `src/model_helper.rs` | `ModelHelper`, `WithValidation`, `ModelValidationError`. |
| `soft_delete.rs` | `src/soft_delete.rs` | `SoftDeletable` + `SoftDeleteExt`. |
| `timestamps.rs` | `src/timestamps.rs` | `Timestamps` trait + `apply_timestamps`, `TimestampFields`, `timestamp_now_rfc3339`. |
| `field_meta.rs` | `src/field_meta.rs` | `FieldMeta`, `FieldType`, `RelationFieldType`, `RelationMeta`, `EntityFields`, `ValidateMeta`, `ValidatorType`. |
| `macros.rs` | `src/macros.rs` | Filter helper constructors + `filter!` / `filters!`. |
| `utils.rs` | `src/utils.rs` | `compare_values`, `generate_id`, `short_id`, `get_document_id`, `DocumentExt`. |
| `id/` | `src/id/` | id strategies and generator. |
| `schema/` | `src/schema/` | `Schema`, `SchemaManager`, `PrefixConfig`, `PrefixHolder`. |
| `nosql_index/` | `src/nosql_index/` | `NosqlIndex`, `NosqlIndexInfo`, `NosqlIndexType`, `IndexManager`. |
| `embedded/` | `src/embedded/` | (feature `embedded`) |
| `inheritance/` | `src/inheritance/` | (feature `inheritance`) |
| `lazy/` | `src/lazy/` | (feature `lazy`) |
| `cache/` | `src/cache/` | (feature `query_cache`) |

### Opt-in extension modules (feature-gated, unreferenced)

`embedded/`, `inheritance/`, `lazy/` are compiled only under their own
feature. Nothing in `Storage`, `Repository` or the providers calls them.

## 12. Notes

- There is no `cli/`, `compiled.rs`, `pool/`, `cdc/`, `graphql/`,
  `subscription/` or `search/` module. Older revisions of this document
  listed them; they do not exist on disk.
- Library passes `cargo check`; run
  `cargo test -p nosql_orm --features json,sql-sqlite --lib` for the
  current unit-test count.