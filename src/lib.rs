//! # nosql_orm
//!
//! A TypeORM-inspired ORM for NoSQL databases.
//! Supports JSON file storage, MongoDB, and Redis with a unified, ergonomic API.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use nosql_orm::prelude::*;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct User {
//!     pub id: Option<String>,
//!     pub name: String,
//!     pub email: String,
//! }
//!
//! impl Entity for User {
//!     fn meta() -> EntityMeta { EntityMeta::new("users") }
//!     fn get_id(&self) -> Option<String> { self.id.clone() }
//!     fn set_id(&mut self, id: String) { self.id = Some(id); }
//! }
//!
//! #[tokio::main]
//! async fn main() -> OrmResult<()> {
//!     let provider = JsonProvider::new("./data").await?;
//!     let repo: Repository<User, _> = Repository::new(provider);
//!     let user = User { id: None, name: "Alice".into(), email: "alice@example.com".into() };
//!     let saved = repo.save(user).await?;
//!     println!("Saved user: {:?}", saved);
//!     Ok(())
//! }
//! ```

pub mod aggregation;
pub mod cascade;
pub mod cdc;
pub mod cli;
pub mod constraints;
pub mod embedded;
pub mod entity;
pub mod error;
pub mod events;
pub mod field_meta;
pub mod graphql;
pub mod id;
pub mod inheritance;
pub mod lazy;
pub mod macros;
pub mod migrations;
pub mod nosql_index;
pub mod pool;
pub mod provider;
pub mod query;
pub mod relations;
pub mod repository;
pub mod schema;
pub mod search;
pub mod soft_delete;
pub mod sql;
pub mod subscription;
pub mod timestamps;
pub mod transaction;
pub mod utils;
pub mod validators;

pub use entity::Entity;
pub use entity::EntityMeta;
pub use entity::FrontendProjection;
pub use timestamps::Timestamps;

pub mod logging;
pub mod providers;

#[cfg(feature = "query_cache")]
pub mod cache;

pub use nosql_orm_derive::Model;
pub use nosql_orm_derive::Validate;

pub use aggregation::{
  Aggregation, AggregationPipeline, GroupStage, LimitStage, MatchStage, ProjectStage, SkipStage,
  SortStage, Stage,
};
pub use cascade::CascadeManager;
pub use cdc::{AuditAction, AuditLog, CdcSync, Change, ChangeCapture, ChangeStream, ChangeType};
pub use constraints::{
  CheckConstraintDef, ColumnConstraint, ColumnDef, ColumnType, IndexDef, IndexType,
  UniqueConstraintDef,
};
pub use embedded::{EmbedExt, Embedded, EmbeddedMeta, Embedder};
pub use events::{EntityEventListener, EntityEvents, Event, EventType};
pub use graphql::{
  GraphQLArg, GraphQLEntity, GraphQLField, GraphQLSchema, GraphQLTypeDef, MutationRoot, QueryRoot,
  SchemaBuilder,
};
pub use inheritance::{Discriminator, DiscriminatorValue, Inheritance, InheritanceType};
pub use lazy::{Lazy, LazyLoader, LazyMany, LazyRelation};
pub use migrations::migration::{JsonMigration, Migration, MigrationMeta, SqlMigration};
pub use migrations::runner::MigrationRunner;
pub use schema::{PrefixConfig, PrefixHolder, Schema, SchemaManager};
pub use search::{
  FullTextIndex, FullTextQueryExt, FullTextSearch, SearchResult, SearchScore, TextSearch,
};
pub use soft_delete::{SoftDeletable, SoftDeleteExt};
pub use subscription::{Publisher, Subscription, SubscriptionHandler, SubscriptionManager, Topic};
pub use validators::{
  EmailValidator, FieldValidator, LengthValidator, PatternValidator, RangeValidator,
  ValidationError, ValidationResult,
};

pub use sql::{
  SqlColumnDef, SqlColumnType, SqlDialect, SqlIndexDef, SqlIndexType, SqlPrimaryKey,
  SqlQueryBuilder, SqlTableDef,
};

/// Re-exports everything you need for typical usage.
pub mod prelude {
  pub use crate::entity::{Entity, EntityMeta, FrontendProjection};
  pub use crate::error::{OrmError, OrmResult};
  pub use crate::field_meta::{
    EntityFieldMeta, EntityFields, FieldMeta, FieldType, RelationFieldType, RelationMeta,
    ValidateMeta, ValidatorType,
  };
  pub use crate::provider::{DatabaseProvider, ProviderConfig};
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

  pub use crate::pool::{JsonPool, Pool, PoolConfig, Pooled};

  #[cfg(feature = "mongo")]
  pub use crate::pool::MongoPool;

  pub use crate::transaction::{Transaction, TransactionState};

  pub use crate::validators::{
    EmailValidator, EntityValidator, EnumValidator, FieldValidator, LengthValidator,
    PatternValidator, RangeValidator, ValidationError, ValidationResult,
  };

  pub use crate::graphql::{
    GraphQLArg, GraphQLEntity, GraphQLField, GraphQLSchema, GraphQLTypeDef, MutationRoot,
    QueryRoot, SchemaBuilder,
  };

  pub use crate::cli;
  pub use crate::logging;

  #[cfg(feature = "redis")]
  pub use crate::providers::redis::RedisProvider;

  pub use crate::cdc::CdcSync;
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
  pub use crate::cache::{CacheConfig, CacheStats, CachedResult, QueryCache};

  pub use crate::filter;
  pub use crate::filters;
  pub use crate::macros::{
    and_filter, eq_filter, gt_filter, gte_filter, in_filter, is_not_null_filter, is_null_filter,
    lt_filter, lte_filter, ne_filter, not_filter, or_filter,
  };
}
