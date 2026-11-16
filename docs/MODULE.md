# nosql_orm — Module Reference

Per-module deep dive. Order mirrors dependency flow: provider trait first,
concrete providers, repository / query, then cross-cutting features.

LOC counts in parentheses, measured 2026-11-14.

## 1. Provider trait layer

### `provider.rs` (568)

```text
ProviderConfig       → connection string + db name + option bag
Capability           → Transactions / Indexes / Aggregation / RawQueries
Storage              → insert / find_by_id / find_many / update / patch /
                       delete / delete_many / update_many / count / aggregate /
                       search / find_stream / insert_many
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
  (defaults to `NotSupported`), `health_check` (defaults `true`),
  `insert_many` (loops `insert`), `search` (in-memory substring match) and
  `find_stream` (collect-then-stream).
- `StorageBackend: Send + Sync`. Blanket impl forwards it to any provider that
  is also `SchemaIntrospection + AdminCommands`; `impl Storage for
  Arc<dyn StorageBackend>` bridges it back to `Repository`.
- `ProviderCapabilities`: `none()` is the conservative default; `all()` and
  `with(capability)` build up from it.
- `ProviderConfig`: connection string + optional database name +
  `options: HashMap<String, String>`.

### `admin_types.rs` (95)

DTOs: `CollectionMeta`, `CollectionSchema`, `CollectionStats`,
`ConnectionHealth`, `FieldInfo`, `IndexInfo`, `PoolStats`, `RawResult`,
`TransactionId`. Re-exported from `provider.rs` and the prelude.

## 2. Concrete providers — `providers/`

### `providers/mod.rs` (56)

Module map only: declares each provider under its feature flag and
re-exports it. `FallbackProvider<P1, P2>` moved to its own child file.

### `providers/fallback.rs` (80)

`FallbackProvider<P1, P2>` with `find_with_fallback` and
`find_all_with_fallback` — tries the primary, falls back on error/timeout.

### `providers/etcd.rs` (698) — etcd key-value store (feature `etcd`, native)

`EtcdProvider` over `etcd-client` (gRPC). Collection = key prefix
`{prefix}{collection}/{id}`; `/` is rejected in collection names so the
prefix stays unambiguous. `insert` is compare-and-swap insert-if-absent,
`update` requires an existing document, `rename_collection` moves keys.
`capabilities()`: none. Indexes, `execute_raw` and transactions all answer
`NotSupported` (etcd's `Txn` is one atomic request, not a session).

### `providers/json.rs` (837) — JSON file storage (feature `json`, default)

`JsonCacheConfig`, `JsonProviderConfig` (env-overridable via
`NOSQL_ORM_DATA_DIR`), `JsonProvider`. Collection name validation
(`validate_collection_name`). LRU-style eviction via `access_order`. Flush on
insert/update. `capabilities()`: `Aggregation` + `Transactions`.

### `providers/json_tree.rs` (571) — one file per document (feature `json`)

`JsonTreeProvider`. `capabilities()`: none.

### `providers/mongo/` (515 + 197 in submodules, feature `mongo`)

`MongoProvider` plus `filter`, `convert`, `helpers`, `indexes` submodules.
`capabilities()`: `Indexes`, `Aggregation`. Extra: `create_mongo_index`,
`drop_mongo_index`. Transactions answer `NotSupported` (would need a session).

### `providers/redis.rs` (508) (feature `redis`)

`RedisProvider` with `with_prefix`. `capabilities()`: none. Indexes
`NotSupported`. Transactions `NotSupported` (see `RedisTxStorage`). Extra:
`publish`.

### `providers/kv_common.rs` (50)

Shared helpers for the key-value providers: `document_id`,
`document_id_or_new`, `select_documents`. Every KV provider filters, sorts
and paginates in memory after the read, so the logic lives here once
instead of being copied per provider.

### `providers/redis_vector.rs` (267) (feature `redis-vector`, native)

`RedisVectorProvider` — Redis storage plus `upsert_with_embedding` /
`search_similar`. Delegates all four contract traits to `RedisProvider` and
delegates the scoring to `VectorSearchStorage`, so the distance maths stays
in one tested place. Works against a plain `redis-server`; no RediSearch.

### `providers/couchdb.rs` (647) (feature `couchdb`, native)

`CouchDbProvider` over `reqwest` — database per collection, `_all_docs` for
reads, basic auth parsed out of the connection URL.

### `providers/dynamodb.rs` (667) (feature `dynamodb`, native)

`DynamoDbProvider` over `reqwest` — partition/sort key model, filter
expressions mapped onto `Filter`.

### `providers/elasticsearch.rs` (694) (feature `elasticsearch`, native)

`ElasticsearchProvider` over `reqwest` — index per collection, `_search`
for reads.

### `providers/sql/` (feature `sql-*`)

`SqliteProvider` (560), `PostgresProvider` (580), `MySqlProvider` (601), plus
`common` (178), `executor` (244), `row` (107), `utils` (364), `mod` (73);
plus the native drivers listed below and `postgres.rs` (629),
`mysql.rs` (650), `sqlite.rs` (610).
`capabilities()`: all three advertise `Indexes`, `Transactions`,
`RawQueries`. Extra: `dialect()`, `query_builder()`, `build_select(...)`.

Native driver providers, each in its own file with its own connection
handling: `duckdb.rs` (607), `mssql.rs` (671), `oracle.rs` (710),
`clickhouse.rs` (823). Wire-compatible aliases stay one-line re-exports:
`CockroachProvider` → `PostgresProvider`, `MariaDbProvider` → `MySqlProvider`.

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
mod.rs (…)                → Repository<E, P>, QueryStream, SyncResult,
                             QueryTimeout, QueryStreamTrait
crud.rs (184)              → insert, update, save, save_from_value,
                             insert_many, update_many, upsert_many,
                             update_with_immutable, patch, delete_many
find.rs (149)              → find_by_id, get_by_id, find_all,
                             find_all_including_deleted, count, exists,
                             find_for_frontend, find_by_id_for_frontend,
                             *_with_timeout
delete.rs (83)             → delete, remove, soft_delete, restore
query.rs (203)             → filter-based queries (RepositoryQuery)
indexes.rs (32)            → create_index, drop_index, list_indexes
relations.rs (236)         → with_relations, eager loading
transaction.rs (…)         → begin/commit/rollback/is_transaction_active
fast_path.rs (103)         → hot-path optimization, exec_batch_by_ids
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
**Known gap:** `SqlMigration`/`JsonMigration` `up`/`down` are no-ops — the
runner records applied versions but does not execute the migration body.
Tracked as coming soon in `07-features/README.md`.

## 10. Error layer — `error.rs` (97)

`OrmError` with 23 variants (see 09-api-reference). Helpers
`map_err_connection`, `map_err_query`; alias `OrmResult<T>`.

## 11. Cross-cutting features

### Always compiled

| Module | Path | Purpose |
|---|---|---|
| `logging/` | `src/logging/` | Query logger, file-backed query logger, db query logger, wrapper. |
| `aggregation/` | `src/aggregation/` | `AggregationPipeline`, `GroupStage`, `MatchStage`, `ProjectStage`, `SortStage`, `LimitStage`, `SkipStage`, accumulators. |
| `constraints/` | `src/constraints/` | `ColumnConstraint`, `CheckConstraintDef`, `UniqueConstraintDef`, `IndexDef`, `IndexType`, `ColumnDef`, `ColumnType`. |
| `events/` | `src/events/` | `EntityEvents`, `EntityEventListener`, `Event`, `EventType`, `InsertEvent`, `UpdateEvent`, `DeleteEvent`, `QueryEvent`. |
| `validators/` | `src/validators/` | `FieldValidator`, `EmailValidator`, `LengthValidator`, `PatternValidator`, `RangeValidator`, `EnumValidator`, `CompositeValidator`, `EntityValidator`, `ValidationError`, `ValidationResult`. |
| `model_helper.rs` | `src/model_helper.rs` | `ModelHelper`, `WithValidation`, `ModelValidationError`. |
| `soft_delete.rs` | `src/soft_delete.rs` | `SoftDeleteable` + `SoftDeleteExt`. |
| `timestamps.rs` | `src/timestamps.rs` | `Timestamps` trait + `apply_timestamps`, `TimestampFields`, `timestamp_now_rfc3339`. |
| `field_meta.rs` | `src/field_meta.rs` | `FieldMeta`, `FieldType`, `RelationFieldType`, `RelationMeta`, `EntityFields`, `ValidateMeta`, `ValidatorType`. |
| `macros.rs` | `src/macros.rs` | Filter helper constructors + `filter!` / `filters!`. |
| `utils.rs` | `src/utils.rs` | `compare_values`, `generate_id`, `short_id`, `get_document_id`, `DocumentExt`. |
| `id/` | `src/id/` | id strategies and generator. |
| `schema/` | `src/schema/` | `Schema`, `SchemaManager`, `PrefixConfig`, `PrefixHolder`. |
| `nosql_index/` | `src/nosql_index/` | `NosqlIndex`, `NosqlIndexInfo`, `NosqlIndexType`, `IndexManager`. |

### Wrapper / feature modules (always compiled)

| Module | Path | Purpose |
|---|---|---|
| `cdc` | `src/cdc.rs` (235) | `ChangeCaptureStorage<P>`, `ChangeEvent`, `ChangeOp` — records insert/update/delete, `drain_changes` / `peek_changes` / `changes_stream`. |
| `subscription` | `src/subscription.rs` (127) | `SubscriptionManager`, `Subscription`, `SubscriptionFilter`, `SubscribedStorage<P>` — in-process fan-out of `ChangeEvent`s. |
| `tenant` | `src/tenant.rs` (195) | `TenantStorage<P>` — injects `_tenant_id` on write, filters every read. |
| `sync` | `src/sync.rs` (143) | `SyncEngine<L, R>`, `SyncOp`, `SyncEntry`, `SyncStatus` — offline write queue drained to a remote store. |
| `encryption` | `src/encryption.rs` (204) | `EncryptedStorage<P>`, `KeyProvider`, `StaticKeyProvider` — AES-256-GCM field encryption. |
| `vector_search` | `src/vector_search.rs` (184) | `VectorSearchStorage<P>`, `DistanceMetric` (Cosine/Euclidean/DotProduct). |
| `mongo_tx` | `src/mongo_tx.rs` (175) | `MongoTxStorage` — write-buffer transactions for Mongo. |
| `redis_tx` | `src/redis_tx.rs` (174) | `RedisTxStorage` — write-buffer transactions for Redis. |
| `pgvector` | `src/pgvector.rs` (170) | `PgVectorProvider` — Postgres + `vector` extension (feature `pgvector`). |
| `new_providers` | `src/new_providers.rs` (52) | Type aliases for every feature-gated provider alias. |

### Feature-gated extension modules

`embedded/` (141), `inheritance/` (111), `lazy/` (279) are compiled only
under their own feature (`embedded`, `inheritance`, `lazy`). Nothing in
`Storage`, `Repository` or the providers calls them — integration is coming
soon.

`cache/` (258) is compiled under `query_cache`.

## 12. Feature flags

```toml
default = ["json"]
json, mongo, redis                    # base providers
sql = ["sql-postgres", "sql-sqlite", "sql-mysql"]
full = [everything above + every provider alias]

# provider features
sql-mssql → tiberius + tokio-util ; sql-oracle → oracle
sql-duckdb → duckdb ; sql-clickhouse → clickhouse
sql-cockroach → sql-postgres ; sql-mariadb → sql-mysql
couchdb, elasticsearch → reqwest ; dynamodb → reqwest
etcd → etcd-client ; influxdb → influxdb (all native)
couchbase, firestore, neo4j, cassandra → mongo
graphql → json
pinecone, weaviate, milvus, qdrant, chroma → mongo
pgvector → sql-postgres ; redis-vector → redis (native wrapper)

# opt-in extensions
query_cache, embedded, inheritance, lazy
```

## 13. Tests

- 41 integration suites in `tests/`; `tests/test_new_features.rs` covers
  transactions, CDC, subscriptions, encryption, vector search, tenant
  isolation, streaming and full-text search (9 tests, hermetic).
- `tests/test_provider_conformance.rs` holds every provider to the same
  `Storage` contract plus rollback behaviour; server-backed suites are
  `#[ignore]`d and skip when the server is unreachable.
- `tests/test_etcd_integration.rs` (9 tests) runs the real contract against
  a live etcd: duplicate rejection, update-requires-existence, bulk counts,
  introspection, capability/transaction honesty, name validation, rename.
  It skips when no server answers:
  `ETCD_ENDPOINTS=http://127.0.0.1:2379 cargo test --features etcd --test test_etcd_integration -- --include-ignored`
- `tests/test_redis_vector_integration.rs` (6 tests) runs against a live
  Redis: nearest-first ordering, `top_k` truncation, all three distance
  metrics, a custom embedding field, malformed vectors being skipped, and
  the inherited Redis storage contract. It skips when no server answers:
  `cargo test --features redis-vector --test test_redis_vector_integration -- --include-ignored`
- `tests/test_new_provider_conformance.rs` asserts every provider alias
  satisfies `Storage + SchemaIntrospection + AdminCommands +
  TransactionControl` (compile-time, per feature flag).
- All 36 feature flags build and pass `cargo test` individually
  (measured 2026-11-16): `json mongo redis sql sql-postgres sql-sqlite
  sql-mysql sql-mssql sql-oracle sql-cockroach sql-duckdb sql-clickhouse
  sql-mariadb couchdb couchbase dynamodb firestore elasticsearch neo4j
  influxdb cassandra etcd graphql pinecone weaviate milvus qdrant chroma
  pgvector redis-vector query_cache embedded inheritance lazy full`.
- Module unit tests live in `src/<module>/tests/mod.rs`; run them with
  `cargo test -p nosql_orm --features json,sql-sqlite --lib`.

## 14. Notes

- There is no `cli/`, `compiled.rs`, `pool/`, `graphql/` or `search/`
  module. `graphql` is a provider **alias** (`GraphQLProvider` =
  `JsonProvider`), not a module. CDC and subscription are top-level files
  (`src/cdc.rs`, `src/subscription.rs`), not directories.
- Older revisions of this document listed `cdc/` and `subscription/` as
  absent; both now exist as files (added 0.10.0).
