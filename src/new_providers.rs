// Stub providers for new databases (coming soon).
//
// Each provider struct is defined here with a `not_implemented` marker.
// Full Storage trait implementations will be added as the drivers are integrated.

// --- New SQL providers ---

#[cfg(feature = "sql-mssql")]
pub struct MssqlProvider;
#[cfg(feature = "sql-oracle")]
pub struct OracleProvider;

// CockroachDB reuses PostgresProvider (same wire protocol)
#[cfg(feature = "sql-cockroach")]
pub type CockroachProvider = crate::providers::sql::postgres::PostgresProvider;

// MariaDB reuses MySqlProvider (same wire protocol)
#[cfg(feature = "sql-mariadb")]
pub type MariaDbProvider = crate::providers::sql::mysql::MySqlProvider;

#[cfg(feature = "sql-duckdb")]
pub struct DuckDbProvider;
#[cfg(feature = "sql-clickhouse")]
pub struct ClickHouseProvider;

// --- New NoSQL providers ---

#[cfg(feature = "couchdb")]
pub struct CouchDbProvider;
#[cfg(feature = "couchbase")]
pub struct CouchbaseProvider;
#[cfg(feature = "dynamodb")]
pub struct DynamoDbProvider;
#[cfg(feature = "firestore")]
pub struct FirestoreProvider;
#[cfg(feature = "elasticsearch")]
pub struct ElasticsearchProvider;
#[cfg(feature = "neo4j")]
pub struct Neo4jProvider;
#[cfg(feature = "influxdb")]
pub struct InfluxDbProvider;
#[cfg(feature = "cassandra")]
pub struct CassandraProvider;
#[cfg(feature = "etcd")]
pub struct EtcdProvider;

// --- GraphQL provider ---

#[cfg(feature = "graphql")]
pub struct GraphQLProvider;

// --- Vector database providers ---

#[cfg(feature = "pinecone")]
pub struct PineconeProvider;
#[cfg(feature = "weaviate")]
pub struct WeaviateProvider;
#[cfg(feature = "milvus")]
pub struct MilvusProvider;
#[cfg(feature = "qdrant")]
pub struct QdrantProvider;
#[cfg(feature = "chroma")]
pub struct ChromaProvider;