# Vector database providers

> Status: **partially implemented natively** — 0.10.0+.
> pgvector and Redis vector run on real drivers; Pinecone/Weaviate/Milvus/
> Qdrant/Chroma are still aliases over `MongoProvider`.

Dedicated vector database providers for semantic search and AI features.
Complements the vector search in `07t-vector-search.md` (which covers
embedding fields inside regular entities); these are standalone vector
stores.

---

## Providers

| Database | Feature flag | Implementation |
|---|---|---|
| pgvector | `pgvector` | ✅ **native** — `PgVectorProvider` (Postgres + `vector` extension) |
| Redis | `redis-vector` | ✅ **native** — `RedisVectorProvider` (embedding stored with the doc) |
| Pinecone | `pinecone` | 🟡 alias over `MongoProvider` |
| Weaviate | `weaviate` | 🟡 alias over `MongoProvider` |
| Milvus | `milvus` | 🟡 alias over `MongoProvider` |
| Qdrant | `qdrant` | 🟡 alias over `MongoProvider` |
| Chroma | `chroma` | 🟡 alias over `MongoProvider` |

An alias exposes the full `Storage`, `SchemaIntrospection` and
`AdminCommands` implementation of the provider it reuses, so the same
contract applies either way. Vector-specific APIs (`upsert_with_embedding`,
`search_similar`) come from the `VectorSearchStorage` wrapper described in
`07t-vector-search.md`.

### Redis (`redis-vector`) — native

`RedisVectorProvider` is a first-class provider over Redis: it stores the
embedding inside the document and answers nearest-neighbour queries directly.

```rust
use nosql_orm::providers::redis_vector::RedisVectorProvider;
use nosql_orm::vector_search::DistanceMetric;

let store = RedisVectorProvider::new("redis://127.0.0.1:6379").await?
  .with_prefix("myapp:")
  .with_embedding_field("_embedding")
  .with_metric(DistanceMetric::Cosine);

store.upsert_with_embedding("notes", doc, vec![0.1, 0.2, 0.3]).await?;
let hits = store.search_similar("notes", &query_vec, 10).await?; // (doc, distance)
```

It implements all four contract traits (`Storage`, `SchemaIntrospection`,
`AdminCommands`, `TransactionControl`) by delegating to `RedisProvider`, so
it is a drop-in replacement — the conformance suite asserts exactly that.

**No RediSearch required.** Scoring is done in process by delegating to
`VectorSearchStorage`, so the store works against a plain `redis-server`;
only HNSW/FLAT index offload is a coming-soon item. Documents with no
embedding, or one of the wrong dimensionality, are skipped rather than
failing the search.

**Tests:** `tests/test_redis_vector_integration.rs` (6 tests) runs against a
live Redis — nearest-first ordering, `top_k` truncation, all three distance
metrics, custom embedding field, skipped malformed vectors, and the inherited
storage contract:

```bash
cargo test --features redis-vector --test test_redis_vector_integration -- --include-ignored
```

## See also

- [07t-vector-search.md](07t-vector-search.md) — vector/semantic search in entities
- [07l-search.md](07l-search.md) — full-text search
- [03-provider.md](../03-provider.md) — providers
