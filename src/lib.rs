//! # nosql_orm
//!
//! A TypeORM-inspired ORM for NoSQL databases.
//! Supports JSON file storage and MongoDB with a unified, ergonomic API.
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

pub mod entity;
pub mod error;
pub mod provider;
pub mod query;
pub mod relations;
pub mod repository;
pub mod utils;

pub mod providers;

/// Re-exports everything you need for typical usage.
pub mod prelude {
  pub use crate::entity::{Entity, EntityMeta};
  pub use crate::error::{OrmError, OrmResult};
  pub use crate::provider::{DatabaseProvider, ProviderConfig};
  pub use crate::query::{Filter, OrderBy, QueryBuilder, SortDirection};
  pub use crate::relations::{
    ManyToMany, ManyToOne, OneToMany, OneToOne, RelationDef, RelationLoader, RelationType,
    RelationValue, WithLoaded, WithRelations,
  };
  pub use crate::repository::{RelationRepository, Repository};

  #[cfg(feature = "json")]
  pub use crate::providers::json::JsonProvider;

  #[cfg(feature = "mongo")]
  pub use crate::providers::mongo::MongoProvider;
}
