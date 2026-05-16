use nosql_orm::logging::db_query_logger::DbQueryLogger;
use nosql_orm::logging::file_query_logger::FileQueryLogger;
use nosql_orm::logging::query_logger::QueryLogger;
use nosql_orm::logging::wrapper::ProviderWrapper;
use nosql_orm::providers::json::JsonProvider;
use std::sync::Arc;

fn create_test_provider() -> JsonProvider {
  let temp_dir = tempfile::TempDir::new().unwrap();
  JsonProvider::new(temp_dir.path()).unwrap()
}

#[tokio::test]
async fn test_query_logger_enable_disable() {
  let provider = create_test_provider();
  let logger = QueryLogger::new(provider);

  assert!(logger.is_enabled().await);

  logger.disable().await;
  assert!(!logger.is_enabled().await);

  logger.enable().await;
  assert!(logger.is_enabled().await);
}

#[tokio::test]
async fn test_query_logger_new() {
  let provider = create_test_provider();
  let _logger = QueryLogger::new(provider);
}

#[tokio::test]
async fn test_query_logger_insert_logs() {
  let provider = create_test_provider();
  let logger = QueryLogger::new(provider);

  let doc = serde_json::json!({"name": "test"});
  let _ = logger.insert("users", doc).await;
}

#[tokio::test]
async fn test_query_logger_find_by_id() {
  let provider = create_test_provider();
  let logger = QueryLogger::new(provider);

  let doc = serde_json::json!({"name": "test"});
  provider.insert("users", doc).await.unwrap();

  let _ = logger.find_by_id("users", "nonexistent").await;
}

#[tokio::test]
async fn test_query_logger_find_many() {
  let provider = create_test_provider();
  let logger = QueryLogger::new(provider);

  let _ = logger
    .find_many("users", None, None, None, None, true)
    .await;
}

#[tokio::test]
async fn test_query_logger_update() {
  let provider = create_test_provider();
  let logger = QueryLogger::new(provider);

  let doc = serde_json::json!({"name": "test"});
  let inserted = provider.insert("users", doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let _ = logger
    .update("users", serde_json::json!({"name": "updated"}), id)
    .await;
}

#[tokio::test]
async fn test_query_logger_delete() {
  let provider = create_test_provider();
  let logger = QueryLogger::new(provider);

  let doc = serde_json::json!({"name": "test"});
  let inserted = provider.insert("users", doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let _ = logger.delete("users", id).await;
}

#[tokio::test]
async fn test_query_logger_find_all() {
  let provider = create_test_provider();
  let logger = QueryLogger::new(provider);

  let _ = logger.find_all("users").await;
}

#[tokio::test]
async fn test_query_logger_count() {
  let provider = create_test_provider();
  let logger = QueryLogger::new(provider);

  let _ = logger.count("users").await;
}

#[tokio::test]
async fn test_query_logger_exists() {
  let provider = create_test_provider();
  let logger = QueryLogger::new(provider);

  let _ = logger.exists("users", "nonexistent").await;
}

#[tokio::test]
async fn test_provider_wrapper_insert() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );
  let wrapper = ProviderWrapper::new(provider, logger);

  let doc = serde_json::json!({"name": "test"});
  let result = wrapper.insert("users", doc).await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_find_by_id() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider.clone()),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );
  let wrapper = ProviderWrapper::new(provider, logger);

  let doc = serde_json::json!({"name": "test"});
  let inserted = wrapper.insert("users", doc).await.unwrap();

  let result = wrapper
    .find_by_id("users", inserted["id"].as_str().unwrap())
    .await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_find_many() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider.clone()),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );
  let wrapper = ProviderWrapper::new(provider, logger);

  let result = wrapper
    .find_many("users", None, None, None, None, true)
    .await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_update() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider.clone()),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );
  let wrapper = ProviderWrapper::new(provider, logger);

  let doc = serde_json::json!({"name": "test"});
  let inserted = wrapper.insert("users", doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let result = wrapper
    .update("users", serde_json::json!({"name": "updated"}), id)
    .await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_delete() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider.clone()),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );
  let wrapper = ProviderWrapper::new(provider, logger);

  let doc = serde_json::json!({"name": "test"});
  let inserted = wrapper.insert("users", doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let result = wrapper.delete("users", id).await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_count() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider.clone()),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );
  let wrapper = ProviderWrapper::new(provider, logger);

  let result = wrapper.count("users").await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_find_all() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider.clone()),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );
  let wrapper = ProviderWrapper::new(provider, logger);

  let result = wrapper.find_all("users").await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_file_query_logger_new() {
  let provider = create_test_provider();
  let _logger = FileQueryLogger::new(
    Arc::new(provider),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );
}

#[tokio::test]
async fn test_file_query_logger_enable_disable() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );

  assert!(logger.is_enabled().await);

  logger.disable().await;
  assert!(!logger.is_enabled().await);

  logger.enable().await;
  assert!(logger.is_enabled().await);
}

#[tokio::test]
async fn test_file_query_logger_log() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );

  logger
    .log_operation("INSERT", "users", &serde_json::json!({}), 10, true)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_db_query_logger_new() {
  let provider = create_test_provider();
  let _logger = DbQueryLogger::new(provider);
}

#[tokio::test]
async fn test_db_query_logger_enable_disable() {
  let provider = create_test_provider();
  let logger = DbQueryLogger::new(provider);

  assert!(logger.is_enabled().await);

  logger.disable().await;
  assert!(!logger.is_enabled().await);

  logger.enable().await;
  assert!(logger.is_enabled().await);
}

#[tokio::test]
async fn test_db_query_logger_log() {
  let provider = create_test_provider();
  let logger = DbQueryLogger::new(provider);

  logger
    .log_operation("INSERT", "users", &serde_json::json!({}), 10, true)
    .await
    .unwrap();
}

#[test]
fn test_logging_mod_exports() {
  use nosql_orm::logging;
  let _ = logging;
}

use nosql_orm::logging::LoggingStrategy;

#[test]
fn test_logging_strategy_trait_object() {
  let strategy: Box<dyn LoggingStrategy> = Box::new(FileQueryLogger::new(
    Arc::new(create_test_provider()),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  ));
  assert!(strategy.log_start("test", "users").await.is_ok());
}

#[test]
fn test_logging_strategy_clone() {
  let provider = create_test_provider();
  let logger = FileQueryLogger::new(
    Arc::new(provider),
    tempfile::TempDir::new().unwrap().path().to_path_buf(),
  );
  let _cloned = logger.clone();
}
