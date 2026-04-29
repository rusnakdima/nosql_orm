use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult<T> {
  pub entity: T,
  pub score: SearchScore,
  pub highlights: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SearchScore(pub f64);

impl SearchScore {
  pub fn new(score: f64) -> Self {
    Self(score)
  }

  pub fn as_f64(&self) -> f64 {
    self.0
  }
}

pub trait TextSearch: Send + Sync {
  fn text_search(
    &self,
    query: &str,
  ) -> impl std::future::Future<Output = crate::error::OrmResult<Vec<SearchResult<serde_json::Value>>>>
       + Send;

  fn text_search_multi(
    &self,
    queries: &[(&str, &str)],
  ) -> impl std::future::Future<Output = crate::error::OrmResult<Vec<SearchResult<serde_json::Value>>>>
       + Send;
}

pub trait FullTextQueryExt {
  fn text_search(&mut self, field: &str, query: &str) -> &mut Self;
  fn text_score(&mut self) -> &mut Self;
  fn with_highlights(&mut self, fragment_size: usize) -> &mut Self;
}
