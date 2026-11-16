// Ranked full-text search tests (feature-independent: JSON provider only).
//
// These pin the behaviour added by `src/search.rs`: `search` returns the
// best match first instead of provider order, `search_ranked` applies field
// weights, and `search_with_analyzer` lets a caller turn stemming off.
//
// They also pin the deliberate semantic change — matching is by *term*, not
// substring. `rus` no longer finds `Rust`, which is the trade every
// provider-native index (`$text`, `tsvector`, FTS5) makes and what makes
// ranking possible at all.

use nosql_orm::provider::Storage;
use nosql_orm::providers::JsonProvider;
use nosql_orm::search::SimpleAnalyzer;
use serde_json::json;
use tempfile::tempdir;

async fn provider() -> (JsonProvider, tempfile::TempDir) {
  let temp = tempdir().unwrap();
  let provider = JsonProvider::new(temp.path()).await.unwrap();
  (provider, temp)
}

// Two documents: one mentions `rust` in the title, the other only in the
// body, and only one mentions both query terms.
async fn seeded() -> (JsonProvider, tempfile::TempDir) {
  let (provider, temp) = provider().await;
  provider
    .insert(
      "docs",
      json!({"id": "title", "title": "Rust programming", "body": "A systems language"}),
    )
    .await
    .unwrap();
  provider
    .insert(
      "docs",
      json!({"id": "body", "title": "Other tools", "body": "written in rust"}),
    )
    .await
    .unwrap();
  provider
    .insert(
      "docs",
      json!({"id": "both", "title": "Rust and cargo", "body": "tooling"}),
    )
    .await
    .unwrap();
  (provider, temp)
}

#[tokio::test]
async fn search_returns_the_best_match_first() {
  let (provider, _temp) = seeded().await;

  // `rust cargo` only fully matches `both`, so it must come first even
  // though it was inserted last.
  let results = provider
    .search("docs", &["title", "body"], "rust cargo")
    .await
    .unwrap();
  assert!(!results.is_empty());
  assert_eq!(
    results[0]["id"],
    "both",
    "the document matching every term must rank first, got {:?}",
    results.iter().map(|d| d["id"].clone()).collect::<Vec<_>>()
  );
}

#[tokio::test]
async fn search_ranked_weights_fields() {
  let (provider, _temp) = seeded().await;

  // A title hit is worth three body hits, so the title document outranks
  // the body one even though both contain `rust` exactly once.
  let hits = provider
    .search_ranked("docs", &[("title", 3.0), ("body", 1.0)], "rust")
    .await
    .unwrap();

  assert_eq!(hits.len(), 3, "all three documents contain rust");
  assert_eq!(hits[0].0["id"], "title");
  assert_eq!(hits[0].1, 3.0, "title hit scores its weight");
  assert_eq!(hits[1].0["id"], "both");
  assert_eq!(hits[1].1, 3.0, "rust is in this title too");
  assert_eq!(hits[2].0["id"], "body");
  assert_eq!(hits[2].1, 1.0, "body hit scores its weight");
}

#[tokio::test]
async fn search_ranked_scores_are_descending() {
  let (provider, _temp) = seeded().await;
  let hits = provider
    .search_ranked("docs", &[("title", 3.0), ("body", 1.0)], "rust")
    .await
    .unwrap();

  for pair in hits.windows(2) {
    assert!(
      pair[0].1 >= pair[1].1,
      "scores must descend: {} then {}",
      pair[0].1,
      pair[1].1
    );
  }
}

// Matching is by term, not substring: `rus` is not a token in any document.
#[tokio::test]
async fn search_matches_whole_terms_not_substrings() {
  let (provider, _temp) = seeded().await;
  let results = provider
    .search("docs", &["title", "body"], "rus")
    .await
    .unwrap();
  assert!(
    results.is_empty(),
    "a partial word must not match, got {:?}",
    results
  );
}

// Stemming is what lets a query for `run` find text that says `running`.
#[tokio::test]
async fn search_stems_by_default() {
  let (provider, _temp) = provider().await;
  provider
    .insert("docs", json!({"id": "1", "body": "he is running late"}))
    .await
    .unwrap();

  let default = provider.search("docs", &["body"], "run").await.unwrap();
  assert_eq!(default.len(), 1, "running must match run by default");

  // ...and the caller can turn that off.
  let exact = provider
    .search_with_analyzer(
      "docs",
      &[("body", 1.0)],
      "run",
      &SimpleAnalyzer::without_stemming(),
    )
    .await
    .unwrap();
  assert!(
    exact.is_empty(),
    "without stemming, `run` must not match `running`, got {:?}",
    exact
  );
}

// A stop-word-only query has nothing to match, so the answer is no results
// — not every document.
#[tokio::test]
async fn search_drops_stop_word_only_queries() {
  let (provider, _temp) = seeded().await;
  let results = provider
    .search("docs", &["title", "body"], "the and of")
    .await
    .unwrap();
  assert!(
    results.is_empty(),
    "a query of only stop-words must match nothing, got {:?}",
    results
  );
}

// A non-string field must not panic the search.
#[tokio::test]
async fn search_ignores_non_string_fields() {
  let (provider, _temp) = provider().await;
  provider
    .insert("docs", json!({"id": "1", "title": 42, "body": "rust"}))
    .await
    .unwrap();

  let results = provider
    .search("docs", &["title", "body"], "rust")
    .await
    .unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0]["id"], "1");
}

#[tokio::test]
async fn search_with_no_matches_returns_empty() {
  let (provider, _temp) = seeded().await;
  let results = provider
    .search("docs", &["title", "body"], "cobol")
    .await
    .unwrap();
  assert!(results.is_empty());
}
