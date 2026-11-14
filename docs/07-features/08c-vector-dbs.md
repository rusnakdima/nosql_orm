# Vector database providers

> Status: **implemented** via existing-provider reuse (aliases) — 0.10.0+.
> Native driver backends for each product remain a future enhancement.

Dedicated vector database providers for semantic search and AI features.
Complements the vector search in `07t-vector-search.md` (which covers
embedding fields inside regular entities); these are standalone vector
stores.

---

## Providers

| Database | Feature flag | Implementation |
|---|---|---|
| Pinecone | `pinecone` | Reuses `MongoProvider` |
| Weaviate | `weaviate` | Reuses `MongoProvider` |
| Milvus | `milvus` | Reuses `MongoProvider` |
| Qdrant | `qdrant` | Reuses `MongoProvider` |
| Chroma | `chroma` | Reuses `MongoProvider` |
| pgvector | `pgvector` | Postgres extension — reuses `PostgresProvider` |
| Redis (RediSearch) | `redis-vector` | Reuses `RedisProvider` |

Each alias exposes the full `Storage`, `SchemaIntrospection` and
`AdminCommands` implementations of the provider it reuses. Vector-specific
APIs (upsert, similarity search, metadata filters) are available through the
`VectorSearchStorage` wrapper in `07t-vector-search.md`.

## See also

- [07t-vector-search.md](07t-vector-search.md) — vector/semantic search in entities
- [07l-search.md](07l-search.md) — full-text search
- [03-provider.md](../03-provider.md) — providers
