# New NoSQL database providers (coming soon)

> Status: **coming soon** — planned for 0.11.0–0.12.0. Not implemented in 0.9.0.

Extend the NoSQL provider family beyond MongoDB/Redis.

---

## Planned providers

| Database | Feature flag | Notes |
|---|---|---|
| CouchDB | `couchdb` | Document store, REST API, built-in sync/replication |
| Couchbase | `couchbase` | High-performance CouchDB successor, N1QL queries |
| DynamoDB | `dynamodb` | AWS serverless, massively scalable key-value/document |
| Firestore | `firestore` | Firebase, mobile-first, real-time listeners |
| Elasticsearch / OpenSearch | `elasticsearch` | Search + analytics engine |
| Neo4j | `neo4j` | Graph database, Cypher queries |
| InfluxDB | `influxdb` | Time-series database |
| Cassandra / ScyllaDB | `cassandra` | Wide-column store, massive write throughput |
| etcd | `etcd` | Distributed key-value, config store |

## Planned work

- Implement `Storage`/`StorageBackend` for each provider.
- Add provider-specific query translation where the generic filter model
  does not map (e.g., DynamoDB partition keys, Neo4j Cypher, InfluxDB Flux).
- Reuse the CDC/subscription infrastructure (coming soon) for providers with
  native change feeds (CouchDB `_changes`, Firestore listeners, DynamoDB
  streams).

## Why coming soon

Each provider needs a driver crate, auth model and query dialect. The
`Storage` trait is designed to be extended; no core changes needed.
Tracked as `nosql_orm` roadmap item.

## See also

- [03-provider.md](../03-provider.md) — providers
- [07n-cdc.md](07n-cdc.md) — change data capture (coming soon)
- [07p-subscription.md](07p-subscription.md) — real-time subscriptions (coming soon)
