// Per-row batch error reporting: BatchResult, insert_many_detailed and
// insert_many_chunked (B3.5).

use nosql_orm::batch::BatchResult;
use nosql_orm::error::OrmError;
use nosql_orm::provider::Storage;
use nosql_orm::providers::JsonProvider;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn insert_many_detailed_reports_each_failed_row() {
  let dir = tempdir().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();

  provider
    .insert("items", json!({"id": "dup", "name": "already here"}))
    .await
    .unwrap();

  // Row 1 collides with the pre-existing doc; row 3 collides with row 0.
  let docs = vec![
    json!({"id": "a", "name": "A"}),
    json!({"id": "dup", "name": "duplicate of existing"}),
    json!({"id": "b", "name": "B"}),
    json!({"id": "a", "name": "duplicate of row 0"}),
  ];

  let result = provider.insert_many_detailed("items", docs).await.unwrap();

  assert_eq!(result.succeeded, 2);
  assert_eq!(result.failed.len(), 2);
  assert_eq!(result.total(), 4);
  assert!(!result.is_success());

  let failed_indices: Vec<usize> = result.failed.iter().map(|(i, _)| *i).collect();
  assert_eq!(failed_indices, vec![1, 3]);
  assert!(matches!(result.failed[0].1, OrmError::Duplicate(_)));
  assert!(matches!(result.failed[1].1, OrmError::Duplicate(_)));

  // The successes really landed, and the pre-existing doc is untouched.
  let a = provider.find_by_id("items", "a").await.unwrap().unwrap();
  assert_eq!(a["name"], "A");
  let b = provider.find_by_id("items", "b").await.unwrap().unwrap();
  assert_eq!(b["name"], "B");
  let dup = provider.find_by_id("items", "dup").await.unwrap().unwrap();
  assert_eq!(dup["name"], "already here");
}

#[tokio::test]
async fn insert_many_chunked_keeps_global_row_indices() {
  let dir = tempdir().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();

  // 6 rows, chunk size 2 -> the failure at original index 4 lands in the
  // third chunk but must still be reported as row 4.
  let docs = vec![
    json!({"id": "r0"}),
    json!({"id": "r1"}),
    json!({"id": "r2"}),
    json!({"id": "r3"}),
    json!({"id": "bad"}),
    json!({"id": "r5"}),
  ];
  provider
    .insert("items", json!({"id": "bad"}))
    .await
    .unwrap();

  let result = provider
    .insert_many_chunked("items", docs, 2)
    .await
    .unwrap();

  assert_eq!(result.succeeded, 5);
  assert_eq!(result.failed.len(), 1);
  assert_eq!(result.failed[0].0, 4);
  assert!(matches!(result.failed[0].1, OrmError::Duplicate(_)));
}

#[tokio::test]
async fn insert_many_chunked_rejects_zero_batch_size() {
  let dir = tempdir().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();

  let err = provider
    .insert_many_chunked("items", vec![json!({"id": "x"})], 0)
    .await
    .unwrap_err();
  assert!(matches!(err, OrmError::InvalidInput(_)));
}

#[tokio::test]
async fn insert_many_chunked_with_no_failures_is_all_success() {
  let dir = tempdir().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();

  let docs: Vec<_> = (0..5).map(|i| json!({"id": format!("n{}", i)})).collect();
  let result = provider
    .insert_many_chunked("items", docs, 2)
    .await
    .unwrap();

  assert!(result.is_success());
  assert_eq!(result.succeeded, 5);
  assert_eq!(result.total(), 5);
  assert_eq!(result.summary(), "5 ok, 0 failed");
}

#[test]
fn batch_result_absorb_shifts_indices_by_offset() {
  let mut merged = BatchResult::default();
  merged.succeeded = 1;

  let mut chunk = BatchResult::default();
  chunk.succeeded = 2;
  chunk
    .failed
    .push((0, OrmError::Duplicate("row".to_string())));
  chunk
    .failed
    .push((1, OrmError::InvalidInput("bad".to_string())));

  merged.absorb(chunk, 4);

  assert_eq!(merged.succeeded, 3);
  assert_eq!(merged.total(), 5);
  let indices: Vec<usize> = merged.failed.iter().map(|(i, _)| *i).collect();
  assert_eq!(indices, vec![4, 5]);
  assert!(merged.summary().contains("2 failed"));
}
