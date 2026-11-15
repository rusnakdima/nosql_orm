// The transaction closure helper: `storage.transaction(|tx| async { ... })`
// commits on Ok and rolls back on Err (B3.4).

use nosql_orm::error::OrmError;
use nosql_orm::provider::{Storage, TransactionControl};
use nosql_orm::providers::JsonProvider;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn transaction_commits_and_returns_the_closure_value() {
  let dir = tempdir().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();
  provider
    .insert("items", json!({"id": "base", "name": "Base"}))
    .await
    .unwrap();

  let worker = provider.clone();
  let outcome = provider
    .transaction(|_tx| async move {
      worker
        .insert("items", json!({"id": "in-tx", "name": "Tx"}))
        .await?;
      Ok(42)
    })
    .await;

  assert_eq!(outcome.unwrap(), 42);
  assert!(!provider.is_transaction_active().await.unwrap());

  let committed = provider.find_by_id("items", "in-tx").await.unwrap();
  assert_eq!(committed.unwrap()["name"], "Tx");
}

#[tokio::test]
async fn transaction_rolls_back_and_reraises_the_closure_error() {
  let dir = tempdir().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();
  provider
    .insert("items", json!({"id": "base", "name": "Base"}))
    .await
    .unwrap();

  let worker = provider.clone();
  let outcome: Result<(), OrmError> = provider
    .transaction(|_tx| async move {
      worker
        .insert("items", json!({"id": "ghost", "name": "Ghost"}))
        .await?;
      Err(OrmError::InvalidInput("boom".to_string()))
    })
    .await;

  // The closure's own error is what the caller sees, unchanged.
  let err = outcome.unwrap_err();
  assert!(matches!(err, OrmError::InvalidInput(_)));
  assert!(err.to_string().contains("boom"));

  // The write is gone and no transaction was left dangling.
  assert!(provider
    .find_by_id("items", "ghost")
    .await
    .unwrap()
    .is_none());
  assert!(!provider.is_transaction_active().await.unwrap());
  let base = provider.find_by_id("items", "base").await.unwrap();
  assert!(base.is_some());
}
