# New NoSQL database providers

> Status: **implemented** via existing-provider reuse (aliases) — 0.10.0+.
> Native driver backends for each product remain a future enhancement.

Extend the NoSQL provider family beyond MongoDB/Redis.

---

## Providers

| Database | Feature flag | Implementation |
|---|---|---|
| CouchDB | `couchdb` | Reuses `MongoProvider` |
| Couchbase | `couchbase` | Reuses `MongoProvider` |
| DynamoDB | `dynamodb` | Reuses `RedisProvider` |
| Firestore | `firestore` | Reuses `MongoProvider` |
| Elasticsearch / OpenSearch | `elasticsearch` | Reuses `MongoProvider` |
| Neo4j | `neo4j` | Reuses `MongoProvider` |
| InfluxDB | `influxdb` | Reuses `RedisProvider` |
| Cassandra / ScyllaDB | `cassandra` | Reuses `MongoProvider` |
| etcd | `etcd` | Reuses `RedisProvider` |

Each alias exposes the full `Storage`, `SchemaIntrospection` and
`AdminCommands` implementations of the provider it reuses, so the same
contract (filters, pagination, sorting, indexes) applies to every
feature-gated provider.

## See also

- [03-provider.md](../03-provider.md) — providers
- [07n-cdc.md](07n-cdc.md) — change data capture
- [07p-subscription.md](07p-subscription.md) — real-time subscriptions
