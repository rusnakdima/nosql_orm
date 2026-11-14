# Vector database providers (coming soon)

> Status: **coming soon** — planned for 0.11.0. Not implemented in 0.9.0.

Dedicated vector database providers for semantic search and AI features.
Complements the vector search coming soon in `07t-vector-search.md` (which
covers embedding fields inside regular entities); these are standalone vector
stores.

---

## Planned providers

| Database | Feature flag | Notes |
|---|---|---|
| Pinecone | `pinecone` | Managed vector DB, serverless |
| Weaviate | `weaviate` | Open-source, hybrid search (vector + keyword) |
| Milvus | `milvus` | Open-source, GPU-accelerated, scalable |
| Qdrant | `qdrant` | Rust-native, high performance, filtering |
| Chroma | `chroma` | Lightweight, embeddable, Python-first but has Rust client |
| pgvector | `pgvector` | Postgres extension — reuse SQL provider |
| Redis (RediSearch) | `redis-vector` | Vector search in Redis Stack |

## Planned API

```rust
let storage = QdrantProvider::new("http://localhost:6333").await?;
let repo = VectorRepository::new(storage, dim = 384);
repo.upsert(id, embedding, metadata).await?;
let results = repo.search(&query_vec, top_k = 10).await?;
```

## Planned features

- `VectorRepository<E>` with upsert, search, delete, update.
- Metadata filtering (combine vector similarity with attribute filters).
- Batch upsert for bulk embedding ingestion.
- Provider-native indexing (HNSW, IVF, etc.).
- Distance metrics: cosine, euclidean, dot product.

## Why coming soon

Each provider needs a client crate and a vector-specific storage abstraction
separate from the document `Storage` trait. Tracked as `nosql_orm` roadmap
item.

## See also

- [07t-vector-search.md](07t-vector-search.md) — vector/semantic search in entities (coming soon)
- [07l-search.md](07l-search.md) — full-text search (coming soon)
- [03-provider.md](../03-provider.md) — providers
