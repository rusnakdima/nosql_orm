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
`Transaction`, `CascadeRestricted { entity, relation }`, `Mongo` (mongo),
`Redis` (redis), `Validation`, `Internal`, `Timeout`, `PoolExhausted`,
`QueryCancelled`, `ConstraintViolation`, `Deadlock`.

---

## Provider constructors

- `JsonProvider::new(base_dir)` / `with_config(config)`
- `JsonTreeProvider::new(base_dir)`
- `MongoProvider::connect(uri, db_name)` / `from_config(&config)`
- `RedisProvider::new(conn_str)` / `with_prefix(prefix)`
- `SqliteProvider::connect(path)` / `from_config(&config)`
- `PostgresProvider::connect(uri)` / `from_config(&config)`
- `MySqlProvider::connect(uri)` / `from_config(&config)`
- `connect_backend(kind, url)` / `connect_by_name(name, url)`

---

## Macros (`nosql_orm_derive`)

`Model`, `OrmEntity`, `Entity`, `ModelHelper`, `Validate`. Attributes listed
in [02-entity.md](02-entity.md) and [07-features/07a-macros.md](07-features/07a-macros.md).

---

## Modules

See [MODULE.md](MODULE.md) for the full module map.