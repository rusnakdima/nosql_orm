# Full-text search

> Status: **implemented** — `Storage::search`, `search_ranked` and
> `search_with_analyzer` ship in 0.9.0+. Provider-native indexes
> (`$text`, `tsvector`, FTS5) are **coming soon**.

Full-text search across entity fields with term analysis and field-weighted
ranking. Every provider answers the same way because the default
implementation analyses and ranks in memory; a provider with a native index
overrides it.

---

## Implemented API

```rust
// Ranked by relevance, best first — fields weighted equally.
storage.search("tasks", &["title", "body"], "rust cargo").await?;

// Field weights: a title hit counts three times as much as a body hit.
storage.search_ranked("tasks", &[("title", 3.0), ("body", 1.0)], "rust").await?;
//   -> Vec<(Value, score)>, highest score first

// Bring your own analyzer (no stemming, custom stop-list, a real engine).
storage.search_with_analyzer(
  "tasks",
  &[("title", 3.0), ("body", 1.0)],
  "running",
  &SimpleAnalyzer::without_stemming(),
).await?;
```

`search` is a thin wrapper over `search_ranked` with every field weighted
`1.0`, returning just the documents.

### Analysis

`Analyzer` (in `src/search.rs`) is the extension point; `SimpleAnalyzer` is
the default:

- lowercases and splits on non-alphanumerics;
- drops stop-words (`the`, `and`, `of`, …) — a query of only stop-words
  returns **no** results rather than every document;
- strips common English suffixes: `studies` → `study`, `walked` → `walk`,
  `cats` → `cat`, then collapses the doubled consonant stripping leaves
  behind so `running` → `runn` → `run` actually matches `run`.

It is deliberately conservative — not a Porter implementation — and says so.
`Analyzer` is where a real one plugs in.

### Ranking

Score is a weighted sum over **distinct** query terms: a term counts once
per document no matter how often it repeats, so keyword stuffing a field
cannot inflate its score. A document matching no term scores zero and is
omitted rather than returned with a meaningless score. Ties keep the
provider's original order, so results are deterministic.

### Matching is by term, not substring

`rus` no longer matches `Rust`. That is the trade every provider-native
index makes (`$text`, `tsvector`, FTS5) and what makes ranking possible.

Tests: `src/search/tests/mod.rs` (16 unit tests over analysis, stemming and
scoring) and `tests/test_search_ranking.rs` (8 tests through a real
provider: ranking order, field weights, descending scores, whole-term
matching, stemming on/off, stop-word queries, non-string fields).

## Coming soon

- Provider-native indexes: Mongo `$text`, Postgres `tsvector`, SQLite FTS5 —
  each would override `search`/`search_ranked` and advertise
  `Capability::RawQueries`-style opt-in.
- Highlight snippets in the results.
- A pluggable analyzer registry (language-specific analyzers, custom
  stemming algorithms).
- BM25 / TF-IDF weighting instead of the flat weighted sum.

## See also

- [07f-indexes.md](07f-indexes.md) — index infrastructure (prerequisite)
- [07m-aggregation.md](07m-aggregation.md) — pipeline queries
