pub mod admin_types;
pub mod aggregation;
pub mod cascade;
pub mod constraints;
#[cfg(feature = "embedded")]
pub mod embedded;
pub mod entity;
pub mod error;
pub mod events;
pub mod field_meta;
pub mod id;
#[cfg(feature = "inheritance")]
pub mod inheritance;
#[cfg(feature = "lazy")]
pub mod lazy;
pub mod macros;
pub mod migrations;
pub mod model_helper;
pub mod nosql_index;
pub mod provider;
pub mod query;
pub mod relations;
pub mod repository;
pub mod schema;
pub mod soft_delete;
pub mod sql;
pub mod redis_tx;
pub mod sync;
pub mod tenant;
pub mod vector_search;
pub mod timestamps;
pub mod utils;
pub mod validators;

pub use model_helper::{ModelHelper, WithValidation};
pub use mongo_tx::MongoTxStorage;
pub use redis_tx::RedisTxStorage;
pub use sync::SyncEngine;
#[cfg(feature = "pgvector")]
pub use pgvector::PgVectorProvider;
pub use tenant::TenantStorage;
pub use vector_search::{DistanceMetric, VectorSearchStorage};
pub use cdc::{ChangeCaptureStorage, ChangeEvent, ChangeOp};
pub use encryption::{EncryptedStorage, KeyProvider, StaticKeyProvider};
pub use subscription::{SubscribedStorage, Subscription, SubscriptionFilter, SubscriptionManager};

pub use entity::Entity;
pub use entity::EntityMeta;
pub use entity::FrontendProjection;
pub use entity::{
  Accessors, CastType, ComputedField, EntityAccessors, EntityMutators, MutatorDef, Mutators,
  MutatorsExecutor,
};
pub use timestamps::Timestamps;

pub mod logging;
pub mod providers;

pub use providers::FallbackProvider;

#[cfg(feature = "query_cache")]
pub mod cache;
pub mod cdc;
pub mod mongo_tx;
pub mod new_providers;
#[cfg(feature = "pgvector")]
pub mod pgvector;
pub mod encryption;
pub mod subscription;

pub use nosql_orm_derive::Model;
pub use nosql_orm_derive::ModelHelper;
pub use nosql_orm_derive::OrmEntity;
pub use nosql_orm_derive::Validate;

pub use aggregation::{
  Aggregation, AggregationPipeline, GroupStage, LimitStage, MatchStage, ProjectStage, SkipStage,
  SortStage, Stage,
};
pub use cascade::CascadeManager;
pub use constraints::{
  CheckConstraintDef, ColumnConstraint, ColumnDef, ColumnType, IndexDef, IndexType,
  UniqueConstraintDef,
};
#[cfg(feature = "embedded")]
pub use embedded::{EmbedExt, Embedded, EmbeddedMeta, Embedder};
pub use events::{EntityEventListener, EntityEvents, Event, EventType};
#[cfg(feature = "inheritance")]
pub use inheritance::{Discriminator, DiscriminatorValue, Inheritance, InheritanceType};
#[cfg(feature = "lazy")]
pub use lazy::{Lazy, LazyLoader, LazyMany, LazyRelation};
pub use migrations::migration::{JsonMigration, Migration, MigrationMeta, SqlMigration};
pub use migrations::runner::MigrationRunner;
pub use schema::{PrefixConfig, PrefixHolder, Schema, SchemaManager};
pub use soft_delete::{SoftDeletable, SoftDeleteExt};
pub use validators::{
  EmailValidator, FieldValidator, LengthValidator, PatternValidator, RangeValidator,
  ValidationError, ValidationResult,
};

pub use sql::{
  SqlColumnDef, SqlColumnType, SqlDialect, SqlIndexDef, SqlIndexType, SqlPrimaryKey,
  SqlQueryBuilder, SqlTableDef,
};

pub mod prelude {
  pub use crate::admin_types::{
    CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth, IndexInfo, PoolStats,
    RawResult, TransactionId,
  };
  pub use crate::entity::{Entity, EntityMeta, FrontendProjection};
  pub use crate::error::{OrmError, OrmResult};
  pub use crate::field_meta::{
    EntityFieldMeta, EntityFields, FieldMeta, FieldType, RelationFieldType, RelationMeta,
    ValidateMeta, ValidatorType,
  };
  pub use crate::provider::{
    AdminCommands, ProviderConfig, SchemaIntrospection, Storage, StorageBackend, TransactionControl,
  };
  pub use crate::query::{Filter, OrderBy, Projection, QueryBuilder, SortDirection};
  pub use crate::relations::{
    get_collection_relations, get_relation_def, register_collection_relations,
    register_relations_for_entity, ManyToMany, ManyToOne, OneToMany, OneToOne, RelationDef,
    RelationLoader, RelationType, RelationValue, WithLoaded, WithRelations,
  };
  pub use crate::repository::{RelationRepository, Repository, SyncResult};
  pub use crate::soft_delete::SoftDeletable;

  #[cfg(feature = "json")]
  pub use crate::providers::json::JsonProvider;

  #[cfg(feature = "json")]
  pub use crate::providers::json_tree::JsonTreeProvider;

  pub use crate::validators::{
    EmailValidator, EntityValidator, EnumValidator, FieldValidator, LengthValidator,
    PatternValidator, RangeValidator, ValidationError, ValidationResult,
  };

  pub use crate::logging;

  #[cfg(feature = "redis")]
  pub use crate::providers::redis::RedisProvider;

  pub use crate::nosql_index::{IndexManager, NosqlIndex, NosqlIndexInfo, NosqlIndexType};
  pub use crate::timestamps::{apply_timestamps, Timestamps};
  pub use crate::Validate;

  #[cfg(feature = "sql-postgres")]
  pub use crate::providers::sql::PostgresProvider;

  #[cfg(feature = "sql-sqlite")]
  pub use crate::providers::sql::SqliteProvider;

  #[cfg(feature = "sql-mysql")]
  pub use crate::providers::sql::MySqlProvider;

  #[cfg(feature = "query_cache")]
  pub use crate::cache::{CacheStats, CachedResult, QueryCache, QueryCacheConfig};

  pub use crate::macros::{
    and_filter, eq_filter, gt_filter, gte_filter, in_filter, is_not_null_filter, is_null_filter,
    lt_filter, lte_filter, ne_filter, not_filter, or_filter,
  };
}
