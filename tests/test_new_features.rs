// Integration tests for newly implemented features:
// transactions, CDC, subscriptions, encryption, vector search,
// multi-tenancy, streaming, full-text search.

use nosql_orm::cdc::ChangeCaptureStorage;
use nosql_orm::encryption::{EncryptedStorage, StaticKeyProvider};
use nosql_orm::provider::{Storage, TransactionControl};
use nosql_orm::subscription::{SubscribedStorage, SubscriptionFilter};
use nosql_orm::tenant::TenantStorage;
use nosql_orm::vector_search::{DistanceMetric, VectorSearchStorage};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_json_transactions_commit() {
  let dir = tempdir().unwrap();
  let provider = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();

  // Insert a base document
  provider
    .insert("items", json!({"id": "base", "name": "Base"}))
    .await
    .unwrap();

  let tx = provider.begin_transaction().await.unwrap();
  assert!(provider.is_transaction_active().await.unwrap());

  // Insert inside transaction
  provider
    .insert("items", json!({"id": "tx-1", "name": "TxItem"}))
    .await
    .unwrap();

  // Commit
  provider.commit_transaction(tx).await.unwrap();
  assert!(!provider.is_transaction_active().await.unwrap());

  // Verify committed
  let found = provider.find_by_id("items", "tx-1").await.unwrap();
  assert!(found.is_some());
  assert_eq!(found.unwrap()["name"], "TxItem");
}

#[tokio::test]
async fn test_json_transactions_rollback() {
  let dir = tempdir().unwrap();
  let provider = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();

  provider
    .insert("items", json!({"id": "base", "name": "Base"}))
    .await
    .unwrap();

  let tx = provider.begin_transaction().await.unwrap();

  provider
    .insert("items", json!({"id": "tx-rollback", "name": "ShouldNotExist"}))
    .await
    .unwrap();

  provider.rollback_transaction(tx).await.unwrap();
  assert!(!provider.is_transaction_active().await.unwrap());

  // Verify rolled back
  let found = provider.find_by_id("items", "tx-rollback").await.unwrap();
  assert!(found.is_none());
}

#[tokio::test]
async fn test_cdc_captures_changes() {
  let dir = tempdir().unwrap();
  let inner = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();
  let cdc = ChangeCaptureStorage::new(inner);

  cdc
    .insert("items", json!({"id": "1", "name": "Item1"}))
    .await
    .unwrap();

  cdc
    .update("items", "1", json!({"id": "1", "name": "Updated"}))
    .await
    .unwrap();

  cdc.delete("items", "1").await.unwrap();

  let changes = cdc.drain_changes();
  assert_eq!(changes.len(), 3);
  assert!(matches!(changes[0].op, nosql_orm::cdc::ChangeOp::Insert));
  assert!(matches!(changes[1].op, nosql_orm::cdc::ChangeOp::Update));
  assert!(matches!(changes[2].op, nosql_orm::cdc::ChangeOp::Delete));
}

#[tokio::test]
async fn test_subscription_receives_events() {
  let dir = tempdir().unwrap();
  let inner = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();
  let sub_storage = SubscribedStorage::new(inner, 100);

  let mut sub = sub_storage.subscribe(SubscriptionFilter::All);

  sub_storage
    .insert("items", json!({"id": "1", "name": "Item1"}))
    .await
    .unwrap();

  sub_storage.poll_changes().await;

  // Give the spawned task time to forward the event
  tokio::time::sleep(std::time::Duration::from_millis(50)).await;
  let event = sub.try_recv();
  assert!(event.is_some(), "expected a change event");
  assert_eq!(event.unwrap().collection, "items");
}

#[tokio::test]
async fn test_encryption_roundtrip() {
  let dir = tempdir().unwrap();
  let inner = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();
  let key = vec![0u8; 32];
  let kp = Arc::new(StaticKeyProvider::new(key).unwrap());
  let enc = EncryptedStorage::new(inner, kp, vec!["secret".to_string()]);

  enc
    .insert("items", json!({"id": "1", "secret": "my-password", "public": "hello"}))
    .await
    .unwrap();

  let found = enc.find_by_id("items", "1").await.unwrap().unwrap();
  assert_eq!(found["secret"], "my-password");
  assert_eq!(found["public"], "hello");

  // Verify stored encrypted
  let raw = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();
  let raw_doc = raw.find_by_id("items", "1").await.unwrap().unwrap();
  assert!(raw_doc["secret"].as_str().unwrap().starts_with("enc:"));
}

#[tokio::test]
async fn test_vector_search() {
  let dir = tempdir().unwrap();
  let inner = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();
  let vs = VectorSearchStorage::new(inner);

  vs.upsert_with_embedding("docs", json!({"id": "1", "title": "Rust"}), vec![1.0, 0.0, 0.0])
    .await
    .unwrap();
  vs.upsert_with_embedding("docs", json!({"id": "2", "title": "Python"}), vec![0.0, 1.0, 0.0])
    .await
    .unwrap();

  let results = vs.search_similar("docs", &[1.0, 0.0, 0.0], 2).await.unwrap();
  assert_eq!(results.len(), 2);
  assert_eq!(results[0].0["title"], "Rust");
}

#[tokio::test]
async fn test_tenant_isolation() {
  let dir = tempdir().unwrap();
  let inner = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();
  let t1 = TenantStorage::new(inner.clone(), "tenant-1");
  let t2 = TenantStorage::new(inner, "tenant-2");

  t1.insert("items", json!({"id": "1", "name": "T1 Item"}))
    .await
    .unwrap();
  t2.insert("items", json!({"id": "2", "name": "T2 Item"}))
    .await
    .unwrap();

  let t1_items = t1.find_all("items").await.unwrap();
  assert_eq!(t1_items.len(), 1);
  assert_eq!(t1_items[0]["name"], "T1 Item");

  let t2_items = t2.find_all("items").await.unwrap();
  assert_eq!(t2_items.len(), 1);
  assert_eq!(t2_items[0]["name"], "T2 Item");
}

#[tokio::test]
async fn test_streaming() {
  let dir = tempdir().unwrap();
  let provider = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();

  for i in 0..5 {
    provider
      .insert("items", json!({"id": format!("{}", i), "val": i}))
      .await
      .unwrap();
  }

  use futures::StreamExt;
  let mut stream = provider.find_stream("items", None);
  let mut count = 0;
  while let Some(Ok(_)) = stream.next().await {
    count += 1;
  }
  assert_eq!(count, 5);
}

#[tokio::test]
async fn test_full_text_search() {
  let dir = tempdir().unwrap();
  let provider = nosql_orm::providers::JsonProvider::new(dir.path()).await.unwrap();

  provider
    .insert("docs", json!({"id": "1", "title": "Rust programming", "body": "Systems language"}))
    .await
    .unwrap();
  provider
    .insert("docs", json!({"id": "2", "title": "Python scripting", "body": "Dynamic language"}))
    .await
    .unwrap();

  let results = provider.search("docs", &["title", "body"], "rust").await.unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0]["title"], "Rust programming");
}