#[cfg(feature = "json")]
pub mod json;

#[cfg(feature = "mongo")]
pub mod mongo;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(any(
  feature = "sql-postgres",
  feature = "sql-sqlite",
  feature = "sql-mysql"
))]
pub mod sql;

#[cfg(feature = "json")]
pub use json::JsonProvider;

#[cfg(feature = "mongo")]
pub use mongo::MongoProvider;

#[cfg(feature = "redis")]
pub use redis::RedisProvider;

#[cfg(feature = "sql-postgres")]
pub use sql::PostgresProvider;

#[cfg(feature = "sql-sqlite")]
pub use sql::SqliteProvider;

#[cfg(feature = "sql-mysql")]
pub use sql::MySqlProvider;
