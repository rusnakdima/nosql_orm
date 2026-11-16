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

#[cfg(feature = "redis-vector")]
pub mod redis_vector;

#[cfg(any(feature = "redis", feature = "etcd", feature = "influxdb", feature = "couchdb", feature = "dynamodb", feature = "elasticsearch"))]
pub mod kv_common;

#[cfg(feature = "etcd")]
pub mod etcd;

#[cfg(feature = "influxdb")]
pub mod influxdb;

#[cfg(feature = "couchdb")]
pub mod couchdb;

#[cfg(feature = "dynamodb")]
pub mod dynamodb;

#[cfg(feature = "elasticsearch")]
pub mod elasticsearch;

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

#[cfg(feature = "redis-vector")]
pub use redis_vector::RedisVectorProvider;

#[cfg(feature = "etcd")]
pub use etcd::EtcdProvider;

#[cfg(feature = "influxdb")]
pub use influxdb::InfluxDbProvider;

#[cfg(feature = "couchdb")]
pub use couchdb::CouchDbProvider;

#[cfg(feature = "dynamodb")]
pub use dynamodb::DynamoDbProvider;

#[cfg(feature = "elasticsearch")]
pub use elasticsearch::ElasticsearchProvider;

#[cfg(feature = "sql-postgres")]
pub use sql::PostgresProvider;

#[cfg(feature = "sql-sqlite")]
pub use sql::SqliteProvider;

#[cfg(feature = "sql-mysql")]
pub use sql::MySqlProvider;
