# Full-Text Search

Text search with scoring.

Opt-in: compiled only with the `search` feature. No code path in `Storage`,
`Repository` or the providers calls this module yet.

---

## FullTextSearch

```rust
pub struct FullTextSearch { query: String }
pub struct TextSearch;
pub struct SearchScore { pub id: String, pub score: f64 }
pub struct SearchResult { pub results: Vec<Value>, pub scores: Vec<SearchScore> }
```

## Usage

```rust
let search = FullTextSearch::new("rust programming");
let results = repo.search(search).await?;
```

See `src/search/search.rs` for implementation.