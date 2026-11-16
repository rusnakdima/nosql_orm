// Full-text search: analysis and ranking (feature-independent).
//
// `Storage::search` used to do a case-insensitive substring test per field
// and return the documents in whatever order the provider produced them.
// That answered "does this text contain that text", not "which document is
// the best match for this query", so this module adds the two things that
// make search useful:
//
//   * an `Analyzer` — turns text into terms (tokenise, drop stop-words,
//     stem), so `running` matches `run` and `the` never matches anything;
//   * ranking — scores every candidate by field weight, so a hit in `title`
//     outranks the same hit buried in `body`.
//
// The default analyzer is deliberately conservative: it strips only the
// common English suffixes and collapses the doubled consonant that stripping
// leaves behind (`running` → `runn` → `run`). It is not a Porter
// implementation and does not claim to be; `Analyzer` is the extension point
// for a real one.
//
// Matching is by *term*, not substring: the query `rus` no longer matches
// `Rust`. That is the same trade every provider-native index makes
// (`$text`, `tsvector`, FTS5), and it is what makes ranking possible at all.

use std::collections::HashSet;

// Turns free text into the terms a query is matched against.
//
// Implementors must be object-safe and shareable, because a provider holds
// one behind `&dyn Analyzer` across an await point.
pub trait Analyzer: Send + Sync {
  // Reduce `text` to the terms worth matching on.
  fn analyze(&self, text: &str) -> Vec<String>;
}

// The default English analyzer: lowercase, split on non-alphanumerics,
// drop stop-words, optionally stem.
#[derive(Debug, Clone)]
pub struct SimpleAnalyzer {
  stop_words: HashSet<String>,
  stemming: bool,
}

// A small, conventional English stop-list. Kept short on purpose: a term
// that carries meaning in a query ("not", "before") should not be dropped
// just because it is common elsewhere.
const DEFAULT_STOP_WORDS: &[&str] = &[
  "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it",
  "no", "not", "of", "on", "or", "such", "that", "the", "their", "then", "there", "these", "they",
  "this", "to", "was", "will", "with",
];

impl Default for SimpleAnalyzer {
  fn default() -> Self {
    Self::new()
  }
}

impl SimpleAnalyzer {
  pub fn new() -> Self {
    Self {
      stop_words: DEFAULT_STOP_WORDS.iter().map(|w| w.to_string()).collect(),
      stemming: true,
    }
  }

  // Match whole words only, with no suffix stripping.
  pub fn without_stemming() -> Self {
    Self {
      stemming: false,
      ..Self::new()
    }
  }

  // Replace the stop-word list entirely.
  pub fn with_stop_words<I, S>(words: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    Self {
      stop_words: words.into_iter().map(Into::into).collect(),
      stemming: true,
    }
  }

  pub fn has_stop_word(&self, word: &str) -> bool {
    self.stop_words.contains(word)
  }
}

impl Analyzer for SimpleAnalyzer {
  fn analyze(&self, text: &str) -> Vec<String> {
    text
      .split(|c: char| !c.is_alphanumeric())
      .filter(|t| !t.is_empty())
      .map(|t| t.to_lowercase())
      .filter(|t| !self.stop_words.contains(t))
      .map(|t| if self.stemming { stem(&t) } else { t })
      .collect()
  }
}

// Conservative English suffix stripping.
//
// Each rule guards on word length so short words are left alone, and the
// doubled-consonant collapse at the end undoes what stripping leaves
// (`running` → `runn` → `run`) — without it `running` and `run` would never
// match each other, which is the whole point of stemming.
fn stem(word: &str) -> String {
  if word.chars().count() >= 5 && word.ends_with("ies") {
    return format!("{}y", &word[..word.len() - 3]);
  }
  if word.chars().count() >= 6 && word.ends_with("ing") {
    return collapse_doubled(&word[..word.len() - 3]);
  }
  if word.chars().count() >= 5 && word.ends_with("ed") {
    return collapse_doubled(&word[..word.len() - 2]);
  }
  if word.chars().count() >= 4
    && word.ends_with('s')
    && !word.ends_with("ss")
    && !word.ends_with("us")
    && !word.ends_with("is")
  {
    return word[..word.len() - 1].to_string();
  }
  word.to_string()
}

// `runn` → `run`, but `bell` and `class` are left alone: only the sounds
// that English actually doubles get collapsed.
fn collapse_doubled(word: &str) -> String {
  let bytes = word.as_bytes();
  if bytes.len() >= 3 {
    let last = bytes[bytes.len() - 1];
    let prev = bytes[bytes.len() - 2];
    if last == prev && !matches!(last, b'l' | b's' | b'z') {
      return word[..word.len() - 1].to_string();
    }
  }
  word.to_string()
}

// One document with its relevance score. Higher is a better match.
#[derive(Debug, Clone)]
pub struct SearchHit {
  pub document: serde_json::Value,
  pub score: f64,
}

// Score `documents` against `query` and return the matches best-first.
//
// `fields` carries the weight of each field, so `&[("title", 3.0),
// ("body", 1.0)]` makes a title hit worth three body hits. A document that
// matches no term at all scores zero and is dropped rather than returned
// with a meaningless score.
//
// Scoring is a plain weighted sum over distinct query terms: a term counts
// once per document no matter how often it repeats, so a field cannot be
// gamed by keyword stuffing.
pub fn rank_documents(
  documents: Vec<serde_json::Value>,
  fields: &[(&str, f64)],
  query: &str,
  analyzer: &dyn Analyzer,
) -> Vec<SearchHit> {
  let mut terms = analyzer.analyze(query);
  if terms.is_empty() {
    return Vec::new();
  }
  terms.sort_unstable();
  terms.dedup();

  let mut hits: Vec<SearchHit> = documents
    .into_iter()
    .filter_map(|doc| {
      let mut score = 0.0f64;
      for term in &terms {
        let mut best = 0.0f64;
        for (field, weight) in fields {
          if let Some(text) = doc.get(*field).and_then(|v| v.as_str()) {
            if analyzer.analyze(text).iter().any(|t| t == term) {
              best = best.max(*weight);
            }
          }
        }
        score += best;
      }
      if score > 0.0 {
        Some(SearchHit {
          document: doc,
          score,
        })
      } else {
        None
      }
    })
    .collect();

  // Best first; ties keep the provider's original order so the result is
  // deterministic rather than dependent on hash iteration.
  hits.sort_by(|a, b| {
    b.score
      .partial_cmp(&a.score)
      .unwrap_or(std::cmp::Ordering::Equal)
  });
  hits
}

#[cfg(test)]
mod tests;
