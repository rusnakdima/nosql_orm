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
pub mod cdc;
pub mod cli;
pub mod constraints;
pub mod embedded;
pub mod entity;
pub mod error;
pub mod events;
pub mod graphql;
pub mod id;
pub mod inheritance;
pub mod lazy;
pub mod migrations;
pub mod pool;
pub mod provider;
pub mod query;
pub mod relations;
pub mod repository;
pub mod schema;
pub mod search;
pub mod soft_delete;
pub mod subscription;
pub mod transaction;
pub mod utils;
pub mod validators;

pub mod providers;

#[cfg(feature = "query_cache")]
pub mod cache;

pub use aggregation::{
  Aggregation, AggregationPipeline, GroupStage, LimitStage, MatchStage, ProjectStage, SkipStage,
  SortStage, Stage,
};
pub use cdc::{AuditAction, AuditLog, Change, ChangeCapture, ChangeStream, ChangeType};
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

/// Re-exports everything you need for typical usage.
pub mod prelude {
  pub use crate::entity::{Entity, EntityMeta};
  pub use crate::error::{OrmError, OrmResult};
  pub use crate::provider::{DatabaseProvider, ProviderConfig};
  pub use crate::query::{Filter, OrderBy, Projection, QueryBuilder, SortDirection};
  pub use crate::relations::{
    ManyToMany, ManyToOne, OneToMany, OneToOne, RelationDef, RelationLoader, RelationType,
    RelationValue, WithLoaded, WithRelations,
  };
  pub use crate::repository::{RelationRepository, Repository};
  pub use crate::soft_delete::SoftDeletable;

  #[cfg(feature = "json")]
  pub use crate::providers::json::JsonProvider;

  #[cfg(feature = "mongo")]
  pub use crate::providers::mongo::MongoProvider;

  #[cfg(feature = "redis")]
  pub use crate::providers::redis::RedisProvider;

  #[cfg(feature = "query_cache")]
  pub use crate::cache::{CacheConfig, CacheStats, CachedResult, QueryCache};
}
