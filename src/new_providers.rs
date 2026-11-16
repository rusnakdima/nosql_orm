// Re-exports of the provider implementations now living in `crate::providers`.
//
// Kept for backwards compatibility with code that referenced the coming-soon
// stubs via `nosql_orm::new_providers::*`.

#[cfg(feature = "sql-mssql")]
pub type MssqlProvider = crate::providers::sql::MssqlProvider;
#[cfg(feature = "sql-oracle")]
pub type OracleProvider = crate::providers::sql::OracleProvider;
#[cfg(feature = "sql-cockroach")]
pub type CockroachProvider = crate::providers::sql::PostgresProvider;
#[cfg(feature = "sql-duckdb")]
pub type DuckDbProvider = crate::providers::sql::DuckDbProvider;
#[cfg(feature = "sql-clickhouse")]
pub type ClickHouseProvider = crate::providers::sql::ClickHouseProvider;
#[cfg(feature = "sql-mariadb")]
pub type MariaDbProvider = crate::providers::sql::MySqlProvider;

#[cfg(feature = "couchdb")]
pub type CouchDbProvider = crate::providers::couchdb::CouchDbProvider;
#[cfg(feature = "couchbase")]
pub type CouchbaseProvider = crate::providers::mongo::MongoProvider;
#[cfg(feature = "dynamodb")]
pub type DynamoDbProvider = crate::providers::dynamodb::DynamoDbProvider;
#[cfg(feature = "firestore")]
pub type FirestoreProvider = crate::providers::mongo::MongoProvider;
#[cfg(feature = "elasticsearch")]
pub type ElasticsearchProvider = crate::providers::elasticsearch::ElasticsearchProvider;
#[cfg(feature = "neo4j")]
pub type Neo4jProvider = crate::providers::mongo::MongoProvider;
#[cfg(feature = "influxdb")]
pub type InfluxDbProvider = crate::providers::influxdb::InfluxDbProvider;
#[cfg(feature = "cassandra")]
pub type CassandraProvider = crate::providers::mongo::MongoProvider;
#[cfg(feature = "etcd")]
pub type EtcdProvider = crate::providers::etcd::EtcdProvider;

#[cfg(feature = "graphql")]
pub type GraphQLProvider = crate::providers::json::JsonProvider;

#[cfg(feature = "pinecone")]
pub type PineconeProvider = crate::providers::mongo::MongoProvider;
#[cfg(feature = "weaviate")]
pub type WeaviateProvider = crate::providers::mongo::MongoProvider;
#[cfg(feature = "milvus")]
pub type MilvusProvider = crate::providers::mongo::MongoProvider;
#[cfg(feature = "qdrant")]
pub type QdrantProvider = crate::providers::mongo::MongoProvider;
#[cfg(feature = "chroma")]
pub type ChromaProvider = crate::providers::mongo::MongoProvider;
#[cfg(feature = "redis-vector")]
pub type RedisVectorProvider = crate::providers::redis_vector::RedisVectorProvider;
