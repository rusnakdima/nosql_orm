// Compile-time conformance checks for the feature-gated providers.
//
// Each provider alias must implement the same trait set as the provider it
// reuses: `Storage`, `SchemaIntrospection`, `AdminCommands` and
// `TransactionControl`. The checks are `#[cfg(feature = "...")]` so the
// default build stays green and `cargo test --all-features` proves every
// alias compiles against the contract.

#[cfg(any(
  feature = "sql-mssql",
  feature = "sql-oracle",
  feature = "sql-duckdb",
  feature = "sql-clickhouse",
  feature = "couchdb",
  feature = "couchbase",
  feature = "dynamodb",
  feature = "firestore",
  feature = "elasticsearch",
  feature = "neo4j",
  feature = "influxdb",
  feature = "cassandra",
  feature = "etcd",
  feature = "graphql",
  feature = "pinecone",
  feature = "weaviate",
  feature = "milvus",
  feature = "qdrant",
  feature = "chroma",
  feature = "redis-vector",
))]
use nosql_orm::provider::{AdminCommands, SchemaIntrospection, Storage, TransactionControl};

#[cfg(any(
  feature = "sql-mssql",
  feature = "sql-oracle",
  feature = "sql-duckdb",
  feature = "sql-clickhouse",
  feature = "couchdb",
  feature = "couchbase",
  feature = "dynamodb",
  feature = "firestore",
  feature = "elasticsearch",
  feature = "neo4j",
  feature = "influxdb",
  feature = "cassandra",
  feature = "etcd",
  feature = "graphql",
  feature = "pinecone",
  feature = "weaviate",
  feature = "milvus",
  feature = "qdrant",
  feature = "chroma",
  feature = "redis-vector",
))]
fn assert_contract<P>()
where
  P: Storage + SchemaIntrospection + AdminCommands + TransactionControl,
{
}

#[cfg(feature = "sql-mssql")]
#[test]
fn mssql_provider_satisfies_contract() {
  assert_contract::<nosql_orm::MssqlProvider>();
}

#[cfg(feature = "sql-oracle")]
#[test]
fn oracle_provider_satisfies_contract() {
  assert_contract::<nosql_orm::OracleProvider>();
}

#[cfg(feature = "sql-duckdb")]
#[test]
fn duckdb_provider_satisfies_contract() {
  assert_contract::<nosql_orm::DuckDbProvider>();
}

#[cfg(feature = "sql-clickhouse")]
#[test]
fn clickhouse_provider_satisfies_contract() {
  assert_contract::<nosql_orm::ClickHouseProvider>();
}

#[cfg(feature = "couchdb")]
#[test]
fn couchdb_provider_satisfies_contract() {
  assert_contract::<nosql_orm::CouchDbProvider>();
}

#[cfg(feature = "couchbase")]
#[test]
fn couchbase_provider_satisfies_contract() {
  assert_contract::<nosql_orm::CouchbaseProvider>();
}

#[cfg(feature = "dynamodb")]
#[test]
fn dynamodb_provider_satisfies_contract() {
  assert_contract::<nosql_orm::DynamoDbProvider>();
}

#[cfg(feature = "firestore")]
#[test]
fn firestore_provider_satisfies_contract() {
  assert_contract::<nosql_orm::FirestoreProvider>();
}

#[cfg(feature = "elasticsearch")]
#[test]
fn elasticsearch_provider_satisfies_contract() {
  assert_contract::<nosql_orm::ElasticsearchProvider>();
}

#[cfg(feature = "neo4j")]
#[test]
fn neo4j_provider_satisfies_contract() {
  assert_contract::<nosql_orm::Neo4jProvider>();
}

#[cfg(feature = "influxdb")]
#[test]
fn influxdb_provider_satisfies_contract() {
  assert_contract::<nosql_orm::InfluxDbProvider>();
}

#[cfg(feature = "cassandra")]
#[test]
fn cassandra_provider_satisfies_contract() {
  assert_contract::<nosql_orm::CassandraProvider>();
}

#[cfg(feature = "etcd")]
#[test]
fn etcd_provider_satisfies_contract() {
  assert_contract::<nosql_orm::EtcdProvider>();
}

#[cfg(feature = "graphql")]
#[test]
fn graphql_provider_satisfies_contract() {
  assert_contract::<nosql_orm::GraphQLProvider>();
}

#[cfg(feature = "pinecone")]
#[test]
fn pinecone_provider_satisfies_contract() {
  assert_contract::<nosql_orm::PineconeProvider>();
}

#[cfg(feature = "weaviate")]
#[test]
fn weaviate_provider_satisfies_contract() {
  assert_contract::<nosql_orm::WeaviateProvider>();
}

#[cfg(feature = "milvus")]
#[test]
fn milvus_provider_satisfies_contract() {
  assert_contract::<nosql_orm::MilvusProvider>();
}

#[cfg(feature = "qdrant")]
#[test]
fn qdrant_provider_satisfies_contract() {
  assert_contract::<nosql_orm::QdrantProvider>();
}

#[cfg(feature = "chroma")]
#[test]
fn chroma_provider_satisfies_contract() {
  assert_contract::<nosql_orm::ChromaProvider>();
}

#[cfg(feature = "redis-vector")]
#[test]
fn redis_vector_provider_satisfies_contract() {
  assert_contract::<nosql_orm::RedisVectorProvider>();
}
