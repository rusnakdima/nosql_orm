//! SQL database providers for nosql_orm.
//!
//! Provides implementations for PostgreSQL, SQLite, and MySQL.

pub mod row;
pub mod utils;

#[cfg(feature = "sql-sqlite")]
pub mod sqlite;

#[cfg(feature = "sql-postgres")]
pub mod postgres;

#[cfg(feature = "sql-mysql")]
pub mod mysql;

#[cfg(feature = "sql-sqlite")]
pub use sqlite::SqliteProvider;

#[cfg(feature = "sql-postgres")]
pub use postgres::PostgresProvider;

#[cfg(feature = "sql-mysql")]
pub use mysql::MySqlProvider;

use crate::error::{OrmError, OrmResult};
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;

pub fn dialect_from_connection(connection: &str) -> SqlDialect {
  if connection.starts_with("postgres://") || connection.starts_with("postgresql://") {
    SqlDialect::PostgreSQL
  } else if connection.starts_with("mysql://") {
    SqlDialect::MySQL
  } else if connection.ends_with(".db")
    || connection.ends_with(".sqlite")
    || connection.contains(":memory:")
  {
    SqlDialect::SQLite
  } else {
    SqlDialect::PostgreSQL
  }
}

pub fn create_query_builder(dialect: SqlDialect) -> SqlQueryBuilder {
  SqlQueryBuilder::new(dialect)
}
