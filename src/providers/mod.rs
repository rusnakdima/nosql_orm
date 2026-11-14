pub mod fallback;
pub use fallback::FallbackProvider;

#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "json")]
pub mod json_tree;
pub mod sync;
// The variant enum only has meaning when both of its arms exist.
#[cfg(all(feature = "json", feature = "mongo"))]
pub mod variant;

pub mod backend;
pub use backend::{connect_backend, connect_by_name, BackendKind, ConnectionParts};

#[cfg(feature = "mongo")]
pub mod mongo;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(any(
  feature = "sql-postgres",
  feature = "sql-sqlite",
  feature = "sql-mysql",
  feature = "sql-mssql", feature = "sql-oracle", feature = "sql-duckdb", feature = "sql-clickhouse"
))]
pub mod sql;
pub use sync::{ConflictResolution, ProviderSync, SyncOptions};

#[cfg(feature = "json")]
pub use json::JsonProvider;

#[cfg(feature = "json")]
pub use json_tree::JsonTreeProvider;

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
