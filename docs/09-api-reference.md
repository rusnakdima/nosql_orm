# API Reference (quick)

This is a quick index of the public API. See the source and the other docs
for details.

---

## Prelude (`nosql_orm::prelude::*`)

Traits: `Entity`, `EntityMeta`, `FrontendProjection`, `SoftDeletable`,
`Storage`, `StorageBackend`, `SchemaIntrospection`, `AdminCommands`,
`TransactionControl`, `Filter`, `OrderBy`, `Projection`, `QueryBuilder`,
`SortDirection`, `RelationDef`, `RelationLoader`, `RelationType`,
`RelationValue`, `WithLoaded`, `WithRelations`, `Repository`,
`RelationRepository`, `SyncResult`, `Validate`, `FieldValidator`,
`EmailValidator`, `LengthValidator`, `PatternValidator`, `RangeValidator`,
`ValidationError`, `ValidationResult`, `NosqlIndex`, `NosqlIndexInfo`,
`NosqlIndexType`, `IndexManager`, `Timestamps`, `TimestampFields`,
`SoftDeleteExt`.

Providers (feature-gated): `JsonProvider`, `JsonTreeProvider`,
`MongoProvider`, `RedisProvider`, `PostgresProvider`, `SqliteProvider`,
`MySqlProvider`, `FallbackProvider`.

Macros: `eq_filter`, `ne_filter`, `gt_filter`, `gte_filter`, `lt_filter`,
`lte_filter`, `in_filter`, `and_filter`, `or_filter`, `not_filter`,
`is_null_filter`, `is_not_null_filter`.

Extensions (feature-gated): `Embedded`, `EmbeddedMeta`, `EmbedExt`, `Embedder`,
`Discriminator`, `DiscriminatorValue`, `Inheritance`, `InheritanceType`,
`Lazy`, `LazyLoader`, `LazyMany`, `LazyRelation`,
`QueryCache`, `CachedResult`, `CacheStats`, `QueryCacheConfig`.

Other: `ModelHelper`, `WithValidation`, `logging`.

Admin DTOs: `CollectionMeta`, `CollectionSchema`, `CollectionStats`,
`ConnectionHealth`, `IndexInfo`, `PoolStats`, `RawResult`, `TransactionId`.

Errors: `OrmError`, `OrmResult`.

---

## Core types

- `Entity` / `EntityMeta` / `FrontendProjection`
- `Storage` / `StorageBackend` / `ProviderConfig` / `ProviderCapabilities` /
  `Capability`
- `SchemaIntrospection` / `AdminCommands` / `TransactionControl`
- `Repository<E, P>` / `RelationRepository<E, P>` / `QueryStream<E, P>` /
  `QueryStreamTrait`
- `QueryBuilder` / `Filter` / `Projection` / `OrderBy` / `SortDirection` /
  `Cursor` / `PaginatedResult<T>`
- `RelationDef` / `RelationType` / `RelationValue` / `WithLoaded<E>` /
  `WithRelations`
- `NosqlIndex` / `NosqlIndexInfo` / `NosqlIndexType` / `IndexManager`
- `SqlColumnDef` / `SqlColumnType` / `SqlDialect` / `SqlQueryBuilder` /
  `SqlTableDef` / `SqlPrimaryKey` / `SqlIndexDef` / `SqlIndexType` /
  `SqlOnDelete`
- `Migration` / `MigrationMeta` / `SqlMigration` / `JsonMigration` /
  `MigrationRunner` / `MigrationSource` / `MigrationLoader` / `SqlFileLoader`
- `OrmError` / `OrmResult<T>`

---

## Error variants

`NotFound`, `Duplicate`, `Serialization`, `Io`, `Provider`, `NotSupported`,
`Relation`, `InvalidQuery`, `InvalidInput`, `Query`, `Connection`,
`Transaction`, `Encryption`, `CascadeRestricted { entity, relation }`,
`Mongo` (mongo), `Redis` (redis), `Validation`, `Internal`, `Timeout`,
`PoolExhausted`, `QueryCancelled`, `ConstraintViolation`, `Deadlock` —
23 variants.

---

## Provider constructors

- `JsonProvider::new(base_dir)` / `with_config(config)`
- `JsonTreeProvider::new(base_dir)`
- `MongoProvider::connect(uri, db_name)` / `from_config(&config)`
- `RedisProvider::new(conn_str)` / `with_prefix(prefix)`
- `EtcdProvider::connect(endpoints)` / `from_config(&config)` / `with_prefix(prefix)`
- `InfluxDbProvider::connect(url, database)` / `from_config(&config)`
- `RedisVectorProvider::connect(url)` / `new(url)` / `from_config(&config)` /
  `with_prefix` / `with_embedding_field` / `with_metric` /
  `upsert_with_embedding` / `search_similar`
- `SqliteProvider::connect(path)` / `from_config(&config)`
- `PostgresProvider::connect(uri)` / `from_config(&config)`
- `MySqlProvider::connect(uri)` / `from_config(&config)`
- `connect_backend(kind, url)` / `connect_by_name(name, url)`

Native driver providers (feature-gated): `MssqlProvider` (tiberius),
`OracleProvider` (oracle), `DuckDbProvider` (duckdb),
`ClickHouseProvider` (clickhouse). Wire-compatible aliases (feature-gated,
reuse an existing provider): `CockroachProvider` (= Postgres),
`MariaDbProvider` (= MySQL), `CouchDbProvider`, `CouchbaseProvider`,
`FirestoreProvider`, `ElasticsearchProvider`, `Neo4jProvider`,
`CassandraProvider` (= Mongo), `DynamoDbProvider`, `InfluxDbProvider`,
`EtcdProvider` (= Redis), `GraphQLProvider` (= JSON), `PineconeProvider`,
`WeaviateProvider`, `MilvusProvider`, `QdrantProvider`, `ChromaProvider`
(= Mongo), `RedisVectorProvider` (= Redis), `PgVectorProvider`.
Each is exported at the crate root and under `nosql_orm::new_providers`.

---

## Wrapper features (always compiled)

- `ChangeCaptureStorage<P>` / `ChangeEvent` / `ChangeOp` — CDC
  (`nosql_orm::cdc`)
- `SubscriptionManager` / `Subscription` / `SubscriptionFilter` /
  `SubscribedStorage<P>` — in-process subscriptions
  (`nosql_orm::subscription`)
- `TenantStorage<P>` — multi-tenancy (`nosql_orm::tenant`)
- `SyncEngine<L, R>` / `SyncOp` / `SyncEntry` / `SyncStatus` — offline sync
  (`nosql_orm::sync`)
- `EncryptedStorage<P>` / `KeyProvider` / `StaticKeyProvider` — encryption
  at rest (`nosql_orm::encryption`)
- `VectorSearchStorage<P>` / `DistanceMetric` — vector search
  (`nosql_orm::vector_search`)
- `MongoTxStorage<P>` / `RedisTxStorage<P>` — write-buffer transactions
  (`begin_transaction()` / `commit_transaction()` / `rollback_transaction()` /
  `is_transaction_active()`)
- `PgVectorProvider` — Postgres + vector extension
  (`connect(uri)`, `with_embedding_field(f)`, `setup_vector()`,
  `search_similar(collection, embedding, top_k)`)
- `Analyzer` / `SimpleAnalyzer` / `rank_documents` / `SearchHit` — full-text
  analysis and ranking (`nosql_orm::search`); on `Storage`:
  `search(collection, fields, query)` (best-first),
  `search_ranked(collection, &[(field, weight)], query)`,
  `search_with_analyzer(collection, fields, query, &analyzer)`)
- `ChangeCaptureStorage` events flow into `SubscriptionManager` via
  `SubscribedStorage::poll_changes()`

---

## Macros (`nosql_orm_derive`)

`Model`, `OrmEntity`, `Entity`, `ModelHelper`, `Validate`. Attributes listed
in [02-entity.md](02-entity.md) and [07-features/07a-macros.md](07-features/07a-macros.md).

---

## Modules

See [MODULE.md](MODULE.md) for the full module map.