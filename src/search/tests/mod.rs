use super::{rank_documents, Analyzer, SimpleAnalyzer};
use serde_json::json;

#[test]
fn simple_analyzer_lowercases_and_splits() {
  let a = SimpleAnalyzer::without_stemming();
  assert_eq!(a.analyze("Hello, World!"), vec!["hello", "world"]);
}

#[test]
fn simple_analyzer_drops_empty_tokens() {
  let a = SimpleAnalyzer::without_stemming();
  assert_eq!(a.analyze("  --  "), Vec::<String>::new());
  assert_eq!(a.analyze(""), Vec::<String>::new());
}

#[test]
fn simple_analyzer_drops_stop_words() {
  let a = SimpleAnalyzer::without_stemming();
  assert_eq!(a.analyze("the cat sat"), vec!["cat", "sat"]);
  // A stop-word that is the whole query leaves nothing to match on.
  assert_eq!(a.analyze("the"), Vec::<String>::new());
}

#[test]
fn stop_word_list_is_replaceable() {
  // `with_stop_words` builds a fresh analyzer with the given list; the
  // stemming default does not matter here because none of these words
  // survive the stop-list.
  let a = SimpleAnalyzer::with_stop_words(["the", "cat"]);
  assert_eq!(a.analyze("the cat sat"), vec!["sat"]);
  assert!(a.has_stop_word("cat"));
  assert!(!a.has_stop_word("sat"));
}

#[test]
fn default_analyzer_stems() {
  let a = SimpleAnalyzer::new();
  assert_eq!(a.analyze("running"), vec!["run"]);
  assert_eq!(a.analyze("run"), vec!["run"]);
  assert_eq!(a.analyze("studies"), vec!["study"]);
  assert_eq!(a.analyze("walked"), vec!["walk"]);
  assert_eq!(a.analyze("cats"), vec!["cat"]);
}

// Short words must be left alone — stripping `bus` or `class` would corrupt
// them. (`is` and `as` never reach stemming at all: both are stop-words,
// which `stop_word_list_is_replaceable` covers.)
#[test]
fn stemming_leaves_short_words_alone() {
  let a = SimpleAnalyzer::new();
  assert_eq!(a.analyze("bus"), vec!["bus"]);
  assert_eq!(a.analyze("class"), vec!["class"]);
  assert_eq!(a.analyze("bell"), vec!["bell"]);
  assert_eq!(a.analyze("gas"), vec!["gas"]);
}

// Without the doubled-consonant collapse `running` → `runn` would never
// match `run`, which is the entire point of stemming.
#[test]
fn stemming_collapses_the_doubled_consonant() {
  let a = SimpleAnalyzer::new();
  assert_eq!(a.analyze("programming"), a.analyze("program"));
  assert_eq!(a.analyze("running"), a.analyze("run"));
  // But `bell`-style doubles that English really does keep stay put.
  assert_eq!(a.analyze("spell"), vec!["spell"]);
}

#[test]
fn analyzer_is_object_safe() {
  // `Storage::search_with_analyzer` takes `&dyn Analyzer`, so this has to
  // compile or the trait signature is wrong.
  let a: &dyn Analyzer = &SimpleAnalyzer::new();
  assert_eq!(a.analyze("cats"), vec!["cat"]);
}

#[test]
fn rank_drops_documents_that_match_nothing() {
  let docs = vec![
    json!({"id": "1", "title": "Rust"}),
    json!({"id": "2", "title": "Go"}),
  ];
  let hits = rank_documents(
    docs,
    &[("title", 1.0)],
    "rust",
    &SimpleAnalyzer::without_stemming(),
  );
  assert_eq!(hits.len(), 1);
  assert_eq!(hits[0].document["id"], "1");
}

// The point of the feature: a title hit outranks the same term buried in
// the body, instead of the provider's arbitrary order.
#[test]
fn rank_orders_by_field_weight() {
  let docs = vec![
    json!({"id": "body-hit", "title": "Other", "body": "rust is fast"}),
    json!({"id": "title-hit", "title": "Rust basics", "body": "systems"}),
  ];
  let hits = rank_documents(
    docs,
    &[("title", 3.0), ("body", 1.0)],
    "rust",
    &SimpleAnalyzer::without_stemming(),
  );
  assert_eq!(hits.len(), 2);
  assert_eq!(hits[0].document["id"], "title-hit");
  assert_eq!(hits[0].score, 3.0);
  assert_eq!(hits[1].score, 1.0);
}

// Every distinct term counts once per document, so repeating a word in a
// field cannot inflate the score.
#[test]
fn rank_counts_each_term_once() {
  let docs = vec![
    json!({"id": "spam", "body": "rust rust rust"}),
    json!({"id": "once", "body": "rust"}),
  ];
  let hits = rank_documents(
    docs,
    &[("body", 1.0)],
    "rust",
    &SimpleAnalyzer::without_stemming(),
  );
  assert_eq!(hits.len(), 2);
  assert_eq!(
    hits[0].score, hits[1].score,
    "repetition must not change the score"
  );
}

// A multi-term query sums across terms, so a document covering more of the
// query outranks one covering a single term.
#[test]
fn rank_sums_across_query_terms() {
  let docs = vec![
    json!({"id": "one", "title": "rust"}),
    json!({"id": "both", "title": "rust and cargo"}),
  ];
  let hits = rank_documents(
    docs,
    &[("title", 1.0)],
    "rust cargo",
    &SimpleAnalyzer::without_stemming(),
  );
  assert_eq!(hits.len(), 2);
  assert_eq!(hits[0].document["id"], "both");
  assert_eq!(hits[0].score, 2.0);
  assert_eq!(hits[1].score, 1.0);
}

// Stemming is what lets a query for `run` find a document that says
// `running` — the reason the analyzer exists.
#[test]
fn rank_matches_through_stemming() {
  let docs = vec![json!({"id": "1", "body": "he is running late"})];
  let hits = rank_documents(docs, &[("body", 1.0)], "run", &SimpleAnalyzer::new());
  assert_eq!(hits.len(), 1, "running must match run");
}

#[test]
fn rank_with_no_usable_query_terms_returns_nothing() {
  let docs = vec![json!({"id": "1", "body": "anything"})];
  // Stop-words only: there is nothing to match, so nothing should match —
  // not every document.
  let hits = rank_documents(
    docs,
    &[("body", 1.0)],
    "the and of",
    &SimpleAnalyzer::without_stemming(),
  );
  assert!(hits.is_empty());
}

#[test]
fn rank_is_stable_for_equal_scores() {
  let docs = vec![
    json!({"id": "a", "title": "same"}),
    json!({"id": "b", "title": "same"}),
    json!({"id": "c", "title": "same"}),
  ];
  let hits = rank_documents(
    docs,
    &[("title", 1.0)],
    "same",
    &SimpleAnalyzer::without_stemming(),
  );
  let ids: Vec<&str> = hits
    .iter()
    .map(|h| h.document["id"].as_str().unwrap())
    .collect();
  assert_eq!(ids, vec!["a", "b", "c"], "ties must keep input order");
}

// Non-string fields are skipped rather than panicking — a document with a
// numeric `title` must not take the search down.
#[test]
fn rank_ignores_non_string_fields() {
  let docs = vec![json!({"id": "1", "title": 42, "body": "rust"})];
  let hits = rank_documents(
    docs,
    &[("title", 3.0), ("body", 1.0)],
    "rust",
    &SimpleAnalyzer::without_stemming(),
  );
  assert_eq!(hits.len(), 1);
  assert_eq!(hits[0].score, 1.0);
}
