use nosql_orm::providers::redis::RedisProvider;

async fn setup_redis_provider() -> OrmResult<RedisProvider> {
  let connection_string = std::env::var("REDIS_CONNECTION_STRING")
    .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
  RedisProvider::new(&connection_string).await
}

fn setup_redis_provider_with_prefix(prefix: &str) -> OrmResult<RedisProvider> {
  Ok(RedisProvider::new("redis://127.0.0.1:6379")?.with_prefix(prefix))
}

#[tokio::test]
async fn test_redis_provider_connection() {
  let provider = setup_redis_provider().await;
  if provider.is_err() {
    return;
  }
  let _ = provider.unwrap();
}

#[tokio::test]
async fn test_redis_with_prefix() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap().with_prefix("test:");
  let _ = provider;
}

#[tokio::test]
async fn test_redis_insert_and_find_by_id() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap();
  let collection = format!("test_users_{}", uuid::Uuid::new_v4());

  let doc = serde_json::json!({
    "name": "Test User",
    "email": "test@example.com"
  });

  let inserted = provider.insert(&collection, doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let found = provider.find_by_id(&collection, id).await.unwrap();
  assert!(found.is_some());
  assert_eq!(found.unwrap()["name"], "Test User");

  let _ = provider.delete(&collection, id).await;
}

#[tokio::test]
async fn test_redis_find_many_empty() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap();

  let results = provider
    .find_many("nonexistent", None, None, None, None, true)
    .await
    .unwrap();
  assert!(results.is_empty());
}

#[tokio::test]
async fn test_redis_find_many_with_filter() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap();
  let collection = format!("test_users_{}", uuid::Uuid::new_v4());

  provider
    .insert(&collection, serde_json::json!({"name": "Alice", "age": 25}))
    .await
    .unwrap();
  provider
    .insert(&collection, serde_json::json!({"name": "Bob", "age": 30}))
    .await
    .unwrap();

  let filter = nosql_orm::query::Filter::Eq("name".to_string(), serde_json::json!("Alice"));
  let results = provider
    .find_many(&collection, Some(&filter), None, None, None, true)
    .await
    .unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0]["name"], "Alice");

  let _ = provider.delete(&collection, "nonexistent").await;
}

#[tokio::test]
async fn test_redis_find_many_with_skip_limit() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap();
  let collection = format!("test_users_{}", uuid::Uuid::new_v4());

  for i in 0..10 {
    provider
      .insert(&collection, serde_json::json!({"index": i}))
      .await
      .unwrap();
  }

  let results = provider
    .find_many(&collection, None, Some(3), Some(4), None, true)
    .await
    .unwrap();
  assert_eq!(results.len(), 4);

  let _ = provider.delete(&collection, "nonexistent").await;
}

#[tokio::test]
async fn test_redis_update() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap();
  let collection = format!("test_users_{}", uuid::Uuid::new_v4());

  let inserted = provider
    .insert(&collection, serde_json::json!({"name": "Alice", "age": 25}))
    .await
    .unwrap();
  let id = inserted["id"].as_str().unwrap();

  let updated = provider
    .update(
      &collection,
      serde_json::json!({"name": "Alice", "age": 26}),
      id,
    )
    .await
    .unwrap();
  assert_eq!(updated["age"], 26);

  let found = provider.find_by_id(&collection, id).await.unwrap().unwrap();
  assert_eq!(found["age"], 26);

  let _ = provider.delete(&collection, id).await;
}

#[tokio::test]
async fn test_redis_delete() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap();
  let collection = format!("test_users_{}", uuid::Uuid::new_v4());

  let inserted = provider
    .insert(&collection, serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();
  let id = inserted["id"].as_str().unwrap();

  let deleted = provider.delete(&collection, id).await.unwrap();
  assert!(deleted);

  let found = provider.find_by_id(&collection, id).await.unwrap();
  assert!(found.is_none());
}

#[tokio::test]
async fn test_redis_count() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap();
  let collection = format!("test_users_{}", uuid::Uuid::new_v4());

  assert_eq!(provider.count(&collection).await.unwrap(), 0);

  provider
    .insert(&collection, serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();
  provider
    .insert(&collection, serde_json::json!({"name": "Bob"}))
    .await
    .unwrap();

  assert_eq!(provider.count(&collection).await.unwrap(), 2);

  let _ = provider.delete(&collection, "nonexistent").await;
}

#[tokio::test]
async fn test_redis_exists() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap();
  let collection = format!("test_users_{}", uuid::Uuid::new_v4());

  let inserted = provider
    .insert(&collection, serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();
  let id = inserted["id"].as_str().unwrap();

  assert!(provider.exists(&collection, id).await.unwrap());
  assert!(!provider.exists(&collection, "nonexistent").await.unwrap());

  let _ = provider.delete(&collection, id).await;
}

#[tokio::test]
async fn test_redis_find_all() {
  let provider_result = setup_redis_provider().await;
  if provider_result.is_err() {
    return;
  }
  let provider = provider_result.unwrap();
  let collection = format!("test_users_{}", uuid::Uuid::new_v4());

  provider
    .insert(&collection, serde_json::json!({"name": "Alice"}))
    .await
    .unwrap();
  provider
    .insert(&collection, serde_json::json!({"name": "Bob"}))
    .await
    .unwrap();

  let results = provider.find_all(&collection).await.unwrap();
  assert_eq!(results.len(), 2);

  let _ = provider.delete(&collection, "nonexistent").await;
}

use nosql_orm::error::OrmResult;
