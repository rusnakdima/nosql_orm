//! SQL types and abstractions for nosql_orm.
//!
//! This module provides SQL-specific types that work alongside the
//! existing `DatabaseProvider` trait to enable SQL database support.

pub mod query;
pub mod types;

pub use query::SqlQueryBuilder;
pub use types::{
  SqlColumnDef, SqlColumnType, SqlDialect, SqlIndexDef, SqlIndexType, SqlPrimaryKey, SqlTableDef,
};
