use tempfile::TempDir;
use tokio::fs;

use nosql_orm::error::OrmError;
use nosql_orm::provider::DatabaseProvider;
use nosql_orm::providers::json::{CacheConfig, JsonProvider, JsonProviderConfig};

async fn create_test_provider() -> (JsonProvider, TempDir) {
  let temp_dir = TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  (provider, temp_dir)
}

async fn create_provider_with_cache(
  max_entries: usize,
  ttl_seconds: Option<u64>,
) -> (JsonProvider, TempDir) {
  let temp_dir = TempDir::new().unwrap();
  let config = JsonProviderConfig::new(temp_dir.path()).with_cache_config(CacheConfig {
    max_entries_per_collection: max_entries,
    ttl_seconds,
  });
  let provider = JsonProvider::with_config(config).await.unwrap();
  (provider, temp_dir)
}

#[tokio::test]
async fn test_json_provider_config_default() {
  let temp_dir = TempDir::new().unwrap();
  let config = JsonProviderConfig::new(temp_dir.path());
  assert_eq!(config.cache_config.max_entries_per_collection, 10000);
  assert_eq!(config.cache_config.ttl_seconds, Some(3600));
}

#[tokio::test]
async fn test_json_provider_config_custom_cache() {
  let temp_dir = TempDir::new().unwrap();
  let config = JsonProviderConfig::new(temp_dir.path()).with_cache_config(CacheConfig {
    max_entries_per_collection: 500,
    ttl_seconds: None,
  });
  assert_eq!(config.cache_config.max_entries_per_collection, 500);
  assert_eq!(config.cache_config.ttl_seconds, None);
}

#[tokio::test]
async fn test_insert_and_find_by_id() {
  let (provider, _temp_dir) = create_test_provider().await;

  let doc = serde_json::json!({
    "name": "Test User",
    "email": "test@example.com"
  });

  let inserted = provider.insert("users", doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let found = provider.find_by_id("users", id).await.unwrap();
  assert!(found.is_some());
  let found = found.unwrap();
  assert_eq!(found["name"], "Test User");
  assert_eq!(found["email"], "test@example.com");
}

#[tokio::test]
async fn test_insert_with_existing_id() {
  let (provider, _temp_dir) = create_test_provider().await;

  let doc = serde_json::json!({
    "id": "custom-id-123",
    "name": "Test User"
  });

  let inserted = provider.insert("users", doc).await.unwrap();
  assert_eq!(inserted["id"], "custom-id-123");

  let found = provider.find_by_id("users", "custom-id-123").await.unwrap();
  assert!(found.is_some());
}

#[tokio::test]
async fn test_insert_duplicate_id() {
  let (provider, _temp_dir) = create_test_provider().await;

  let doc1 = serde_json::json!({
    "id": "dup-id",
    "name": "First"
  });

  let doc2 = serde_json::json!({
    "id": "dup-id",
    "name": "Second"
  });

  provider.insert("users", doc1).await.unwrap();
  let result = provider.insert("users", doc2).await;
  assert!(result.is_err());
  assert!(matches!(result.unwrap_err(), OrmError::Duplicate(_)));
}

#[tokio::test]
async fn test_find_by_id_not_found() {
  let (provider, _temp_dir) = create_test_provider().await;

  let found = provider.find_by_id("users", "nonexistent").await.unwrap();
  assert!(found.is_none());
}

#[tokio::test]
async fn test_find_many_empty_collection() {
  let (provider, _temp_dir) = create_test_provider().await;

  let results = provider
    .find_many("users", None, None, None, None, true)
    .await
    .unwrap();
  assert!(results.is_empty());
}

#[tokio::test]
async fn test_find_many_with_filter() {
  let (provider, _temp_dir) = create_test_provider().await;

  provider
    .insert("users", serde_json::json!({"name": "Alice", "age": 25}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Bob", "age": 30}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Charlie", "age": 25}))
    .await
    .unwrap();

  let filter = nosql_orm::query::Filter::Eq("age".to_string(), serde_json::json!(25));
  let results = provider
    .find_many("users", Some(&filter), None, None, None, true)
    .await
    .unwrap();
  assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_find_many_with_skip_limit() {
  let (provider, _temp_dir) = create_test_provider().await;

  for i in 0..10 {
    provider
      .insert("users", serde_json::json!({"name": format!("User{}", i)}))
      .await
      .unwrap();
  }

  let results = provider
    .find_many("users", None, Some(3), Some(4), None, true)
    .await
    .unwrap();
  assert_eq!(results.len(), 4);
}

#[tokio::test]
async fn test_find_all() {
  let (provider, _temp_dir) = create_test_provider().await;

  provider
    .insert("users", serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Bob"}))
    .await
    .unwrap();

  let results = provider.find_all("users").await.unwrap();
  assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_update() {
  let (provider, _temp_dir) = create_test_provider().await;

  let inserted = provider
    .insert("users", serde_json::json!({"name": "Alice", "age": 25}))
    .await
    .unwrap();
  let id = inserted["id"].as_str().unwrap();

  let updated = provider
    .update("users", id, serde_json::json!({"name": "Alice", "age": 26}))
    .await
    .unwrap();
  assert_eq!(updated["age"], 26);

  let found = provider.find_by_id("users", id).await.unwrap().unwrap();
  assert_eq!(found["age"], 26);
}

#[tokio::test]
async fn test_update_not_found() {
  let (provider, _temp_dir) = create_test_provider().await;

  let result = provider
    .update("users", "nonexistent", serde_json::json!({"name": "Test"}))
    .await;
  assert!(result.is_err());
}

#[tokio::test]
async fn test_delete() {
  let (provider, _temp_dir) = create_test_provider().await;

  let inserted = provider
    .insert("users", serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();
  let id = inserted["id"].as_str().unwrap();

  let deleted = provider.delete("users", id).await.unwrap();
  assert!(deleted);

  let found = provider.find_by_id("users", id).await.unwrap();
  assert!(found.is_none());
}

#[tokio::test]
async fn test_delete_not_found() {
  let (provider, _temp_dir) = create_test_provider().await;

  let result = provider.delete("users", "nonexistent").await;
  assert!(result.is_err());
}

#[tokio::test]
async fn test_count() {
  let (provider, _temp_dir) = create_test_provider().await;

  assert_eq!(provider.count("users", None).await.unwrap(), 0);

  provider
    .insert("users", serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Bob"}))
    .await
    .unwrap();

  assert_eq!(provider.count("users", None).await.unwrap(), 2);
}

#[tokio::test]
async fn test_exists() {
  let (provider, _temp_dir) = create_test_provider().await;

  let inserted = provider
    .insert("users", serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();
  let id = inserted["id"].as_str().unwrap();

  assert!(provider.exists("users", id).await.unwrap());
  assert!(!provider.exists("users", "nonexistent").await.unwrap());
}

#[tokio::test]
async fn test_clear_cache() {
  let (provider, _temp_dir) = create_test_provider().await;

  provider
    .insert("users", serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();

  let all_docs = provider.find_all("users").await.unwrap();
  let id = all_docs[0]["id"].as_str().unwrap();
  let _ = provider.find_by_id("users", id).await.unwrap();

  provider.clear_cache().await.unwrap();

  let results = provider.find_all("users").await.unwrap();
  assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_validate_collection_name_valid() {
  let (provider, _temp_dir) = create_test_provider().await;

  provider
    .insert("valid_collection_name", serde_json::json!({"test": true}))
    .await
    .unwrap();
  let results = provider.find_all("valid_collection_name").await.unwrap();
  assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_validate_collection_name_empty() {
  let (provider, _temp_dir) = create_test_provider().await;

  let result = provider.insert("", serde_json::json!({"test": true})).await;
  assert!(result.is_err());
  assert!(matches!(result.unwrap_err(), OrmError::InvalidInput(msg) if msg.contains("empty")));
}

#[tokio::test]
async fn test_validate_collection_name_path_traversal() {
  let (provider, _temp_dir) = create_test_provider().await;

  let result = provider
    .insert("../etc/passwd", serde_json::json!({"test": true}))
    .await;
  assert!(result.is_err());
  assert!(
    matches!(result.unwrap_err(), OrmError::InvalidInput(msg) if msg.contains("path traversal"))
  );
}

#[tokio::test]
async fn test_validate_collection_name_with_dot_prefix() {
  let (provider, _temp_dir) = create_test_provider().await;

  let result = provider
    .insert(".hidden", serde_json::json!({"test": true}))
    .await;
  assert!(result.is_err());
  assert!(matches!(result.unwrap_err(), OrmError::InvalidInput(msg) if msg.contains("dot")));
}

#[tokio::test]
async fn test_validate_collection_name_too_long() {
  let (provider, _temp_dir) = create_test_provider().await;

  let long_name = "a".repeat(256);
  let result = provider
    .insert(&long_name, serde_json::json!({"test": true}))
    .await;
  assert!(result.is_err());
  assert!(matches!(result.unwrap_err(), OrmError::InvalidInput(msg) if msg.contains("255")));
}

#[tokio::test]
async fn test_flush_persists_to_disk() {
  let (provider, _temp_dir) = create_test_provider().await;

  provider
    .insert("users", serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();

  drop(provider);

  let content = fs::read_to_string(_temp_dir.path().join("users.json"))
    .await
    .unwrap();
  let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
  assert!(parsed.is_array());
  assert_eq!(parsed.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_cache_eviction_when_full() {
  let (provider, _temp_dir) = create_provider_with_cache(3, None).await;

  for i in 0..5 {
    provider
      .insert("users", serde_json::json!({"index": i}))
      .await
      .unwrap();
  }

  let results = provider.find_all("users").await.unwrap();
  assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_malformed_json_file() {
  let temp_dir = TempDir::new().unwrap();
  let path = temp_dir.path().join("users.json");
  tokio::fs::write(&path, "not valid json {").await.unwrap();

  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let results = provider.find_all("users").await;
  assert!(results.is_err());
}

#[tokio::test]
async fn test_concurrent_inserts() {
  use tokio::task::JoinSet;

  let (provider, _temp_dir) = create_test_provider().await;

  let mut set = JoinSet::new();
  for i in 0..10 {
    let provider = provider.clone();
    set.spawn(async move {
      provider
        .insert("users", serde_json::json!({"index": i}))
        .await
    });
  }

  let mut successes = 0;
  while let Some(result) = set.join_next().await {
    if result.unwrap().is_ok() {
      successes += 1;
    }
  }
  assert_eq!(successes, 10);

  let count = provider.count("users", None).await.unwrap();
  assert_eq!(count, 10);
}

#[tokio::test]
async fn test_find_many_with_and_filter() {
  let (provider, _temp_dir) = create_test_provider().await;

  provider
    .insert(
      "users",
      serde_json::json!({"name": "Alice", "age": 25, "active": true}),
    )
    .await
    .unwrap();
  provider
    .insert(
      "users",
      serde_json::json!({"name": "Bob", "age": 30, "active": true}),
    )
    .await
    .unwrap();
  provider
    .insert(
      "users",
      serde_json::json!({"name": "Charlie", "age": 25, "active": false}),
    )
    .await
    .unwrap();

  let filter = nosql_orm::query::Filter::And(vec![
    nosql_orm::query::Filter::Eq("age".to_string(), serde_json::json!(25)),
    nosql_orm::query::Filter::Eq("active".to_string(), serde_json::json!(true)),
  ]);

  let results = provider
    .find_many("users", Some(&filter), None, None, None, true)
    .await
    .unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0]["name"], "Alice");
}

#[tokio::test]
async fn test_find_many_with_or_filter() {
  let (provider, _temp_dir) = create_test_provider().await;

  provider
    .insert("users", serde_json::json!({"name": "Alice", "age": 25}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Bob", "age": 30}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Charlie", "age": 35}))
    .await
    .unwrap();

  let filter = nosql_orm::query::Filter::Or(vec![
    nosql_orm::query::Filter::Eq("age".to_string(), serde_json::json!(25)),
    nosql_orm::query::Filter::Eq("age".to_string(), serde_json::json!(30)),
  ]);

  let results = provider
    .find_many("users", Some(&filter), None, None, None, true)
    .await
    .unwrap();
  assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_find_many_with_gt_filter() {
  let (provider, _temp_dir) = create_test_provider().await;

  provider
    .insert("users", serde_json::json!({"name": "Alice", "age": 25}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Bob", "age": 30}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Charlie", "age": 35}))
    .await
    .unwrap();

  let filter = nosql_orm::query::Filter::Gt("age".to_string(), serde_json::json!(25));

  let results = provider
    .find_many("users", Some(&filter), None, None, None, true)
    .await
    .unwrap();
  assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_find_many_with_lt_filter() {
  let (provider, _temp_dir) = create_test_provider().await;

  provider
    .insert("users", serde_json::json!({"name": "Alice", "age": 25}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Bob", "age": 30}))
    .await
    .unwrap();
  provider
    .insert("users", serde_json::json!({"name": "Charlie", "age": 35}))
    .await
    .unwrap();

  let filter = nosql_orm::query::Filter::Lt("age".to_string(), serde_json::json!(35));

  let results = provider
    .find_many("users", Some(&filter), None, None, None, true)
    .await
    .unwrap();
  assert_eq!(results.len(), 2);
}
