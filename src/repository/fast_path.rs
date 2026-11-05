use crate::error::OrmResult;
use crate::provider::Storage;
use serde_json::Value;
use std::collections::HashMap;

pub struct FastQueryExecutor;

impl FastQueryExecutor {
  pub async fn exec_single<P: Storage>(
    provider: &P,
    collection: &str,
    id: &str,
  ) -> OrmResult<Option<Value>> {
    provider.find_by_id(collection, id).await
  }

  pub async fn exec_count<P: Storage>(
    provider: &P,
    collection: &str,
    filter: Option<&crate::query::Filter>,
  ) -> OrmResult<u64> {
    provider.count(collection, filter).await
  }

  pub async fn exec_exists<P: Storage>(
    provider: &P,
    collection: &str,
    id: &str,
  ) -> OrmResult<bool> {
    provider.exists(collection, id).await
  }

  pub async fn exec_batch_by_ids<P: Storage>(
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

  pub async fn exec_find_simple<P: Storage>(
    provider: &P,
    collection: &str,
    filter: Option<&crate::query::Filter>,
    limit: Option<u64>,
  ) -> OrmResult<Vec<Value>> {
    provider
      .find_many(collection, filter, None, limit, None, true)
      .await
  }

  pub async fn exec_delete_single<P: Storage>(
    provider: &P,
    collection: &str,
    id: &str,
  ) -> OrmResult<bool> {
    provider.delete(collection, id).await
  }

  pub async fn exec_insert_single<P: Storage>(
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

pub trait FastPathProvider: Storage {
  fn supports_fast_path() -> bool {
    false
  }

  #[allow(async_fn_in_trait)]
  async fn exec_fast_single(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    self.find_by_id(collection, id).await
  }
}

impl<P: Storage> FastPathProvider for P {}

#[cfg(test)]
mod tests;
