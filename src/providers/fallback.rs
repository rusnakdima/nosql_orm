use crate::error::OrmResult;
use crate::provider::Storage;
use crate::query::Filter;
use serde_json::Value;
use std::time::Duration;

pub struct FallbackProvider<P1: Storage, P2: Storage> {
  pub primary: P1,
  pub fallback: P2,
}

impl<P1: Storage + Send + Sync, P2: Storage + Send + Sync> FallbackProvider<P1, P2> {
  pub async fn find_with_fallback(
    &self,
    collection: &str,
    filter: &Filter,
    timeout: Duration,
  ) -> OrmResult<Option<Value>> {
    match tokio::time::timeout(
      timeout,
      self
        .primary
        .find_many(collection, Some(filter), None, Some(1), None, true),
    )
    .await
    {
      Ok(Ok(ref results)) if !results.is_empty() => {
        return Ok(results.first().cloned());
      }
      Ok(Ok(_)) => {}
      Ok(Err(e)) => {
        tracing::warn!("Primary provider error: {}", e);
      }
      Err(_) => {
        tracing::warn!("Primary provider timeout");
      }
    }

    let results = self
      .fallback
      .find_many(collection, Some(filter), None, Some(1), None, true)
      .await?;
    Ok(results.into_iter().next())
  }

  pub async fn find_all_with_fallback(
    &self,
    collection: &str,
    filter: Option<&Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    timeout: Duration,
  ) -> OrmResult<Vec<Value>> {
    match tokio::time::timeout(
      timeout,
      self
        .primary
        .find_many(collection, filter, skip, limit, None, true),
    )
    .await
    {
      Ok(Ok(results)) => {
        if !results.is_empty() {
          return Ok(results);
        }
      }
      Ok(Err(e)) => {
        tracing::warn!("Primary provider error: {}", e);
      }
      Err(_) => {
        tracing::warn!("Primary provider timeout");
      }
    }

    self
      .fallback
      .find_many(collection, filter, skip, limit, None, true)
      .await
  }
}
