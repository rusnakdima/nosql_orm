# Vector / semantic search (coming soon)

> Status: **coming soon** — planned for 0.11.0. Not implemented in 0.9.0.

Vector (semantic) search stores embeddings alongside entities and supports
nearest-neighbour queries. Powers AI features in Forge (voice assistant,
semantic task search, RAG over app content).

---

## Planned API

```rust
#[derive(Entity)]
struct Note {
    id: String,
    title: String,
    body: String,
    #[vector(dim = 384)]
    embedding: Vec<f32>,
}

// query
repo.nearest("embedding", &query_vec, 10).await?;
```

## Planned features

- `#[vector(dim)]` field attribute (derive macro).
- Cosine / euclidean / dot-product distance.
- Provider-native where available (Mongo Atlas Vector Search, Postgres
  pgvector, in-memory brute force for JSON/SQLite).
- Batch embedding generation hook (caller supplies embeddings).

## Why coming soon

Requires a new field type, derive-macro support and provider-specific index
strategies. Tracked as `nosql_orm` roadmap item.

## See also

- [07l-search.md](07l-search.md) — full-text search (coming soon)
- [07f-indexes.md](07f-indexes.md) — index infrastructure
EOF