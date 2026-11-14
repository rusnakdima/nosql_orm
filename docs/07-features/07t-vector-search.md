# Vector / semantic search

> Status: **implemented** — `VectorSearchStorage<P>` + `DistanceMetric`
> ship in 0.10.0. A `#[vector]` derive attribute and provider-native vector
> indexes are **coming soon**.

Vector search stores embeddings alongside entities and answers
nearest-neighbour queries. Powers AI features in Forge (voice assistant,
semantic task search, RAG over app content).

---

## Implemented API

```rust
use nosql_orm::vector_search::{DistanceMetric, VectorSearchStorage};

let store = VectorSearchStorage::new(inner)
  .with_embedding_field("_embedding")
  .with_metric(DistanceMetric::Cosine);

store.upsert_with_embedding("notes", doc, &embedding).await?;
let hits: Vec<(Value, f64)> =
  store.search_similar("notes", &query_vec, 10).await?;   // (doc, distance)
```

- `DistanceMetric`: `Cosine`, `Euclidean`, `DotProduct` — each exposes
  `distance(a: &[f64], b: &[f64]) -> f64`.
- `search_similar` scores every document by its embedding field, sorts by
  ascending distance and truncates to `top_k`. Documents whose embedding is
  missing or of the wrong length are skipped, not errored.
- `PgVectorProvider` (feature `pgvector`) additionally pushes the search
  into PostgreSQL via the `vector` extension — see
  [08c-vector-dbs.md](08c-vector-dbs.md).

Tested by `tests/test_new_features.rs::test_vector_search`.

## Coming soon

- `#[vector(dim = 384)]` field attribute in `nosql_orm_derive`.
- Provider-native indexes (Mongo Atlas Vector Search, pgvector already
  covered), HNSW/IVF build options.
- Batch embedding-generation hook (caller supplies embeddings today).
- `VectorRepository<E>` convenience wrapper over upsert/search/delete.

## See also

- [07l-search.md](07l-search.md) — full-text search
- [07f-indexes.md](07f-indexes.md) — index infrastructure
