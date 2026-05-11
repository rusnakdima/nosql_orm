use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use serde_json::Value;
use std::collections::HashMap;

pub struct FastQueryExecutor;

impl FastQueryExecutor {
  pub async fn exec_single<P: DatabaseProvider>(
    provider: &P,
    collection: &str,
    id: &str,
  ) -> OrmResult<Option<Value>> {
    provider.find_by_id(collection, id).await
  }

  pub async fn exec_count<P: DatabaseProvider>(
    provider: &P,
    collection: &str,
    filter: Option<&crate::query::Filter>,
  ) -> OrmResult<u64> {
    provider.count(collection, filter).await
  }

  pub async fn exec_exists<P: DatabaseProvider>(
    provider: &P,
    collection: &str,
    id: &str,
  ) -> OrmResult<bool> {
    provider.exists(collection, id).await
  }

  pub async fn exec_batch_by_ids<P: DatabaseProvider>(
    provider: &P,
    collection: &str,
    ids: &[String],
  ) -> OrmResult<HashMap<String, Value>> {
    let mut results = HashMap::new();
    for id in ids {
      match provider.find_by_id(collection, id).await {
        Ok(Some(doc)) => {
          results.insert(id.clone(), doc);
        }
        Ok(None) => {}
        Err(e) => return Err(e),
      }
    }
    Ok(results)
  }

  pub async fn exec_find_simple<P: DatabaseProvider>(
    provider: &P,
    collection: &str,
    filter: Option<&crate::query::Filter>,
    limit: Option<u64>,
  ) -> OrmResult<Vec<Value>> {
    provider
      .find_many(collection, filter, None, limit, None, true)
      .await
  }

  pub async fn exec_delete_single<P: DatabaseProvider>(
    provider: &P,
    collection: &str,
    id: &str,
  ) -> OrmResult<bool> {
    provider.delete(collection, id).await
  }

  pub async fn exec_insert_single<P: DatabaseProvider>(
    provider: &P,
    collection: &str,
    doc: Value,
  ) -> OrmResult<Value> {
    provider.insert(collection, doc).await
  }

  pub fn is_simple_filter(filter: &crate::query::Filter) -> bool {
    matches!(
      filter,
      crate::query::Filter::Eq(_, _)
        | crate::query::Filter::In(_, _)
        | crate::query::Filter::IsNull(_)
        | crate::query::Filter::IsNotNull(_)
    )
  }
}

pub trait FastPathProvider: DatabaseProvider {
  fn supports_fast_path() -> bool {
    false
  }

  async fn exec_fast_single(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    self.find_by_id(collection, id).await
  }
}

impl<P: DatabaseProvider> FastPathProvider for P {}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn test_simple_filter_check() {
    let eq_filter = crate::query::Filter::Eq("id".to_string(), serde_json::json!("123"));
    assert!(FastQueryExecutor::is_simple_filter(&eq_filter));

    let and_filter = crate::query::Filter::And(vec![eq_filter.clone()]);
    assert!(!FastQueryExecutor::is_simple_filter(&and_filter));
  }
}
