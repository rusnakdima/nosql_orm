use nosql_orm::logging::db_query_logger::DbQueryLogger;
use nosql_orm::logging::file_query_logger::FileQueryLogger;
use nosql_orm::logging::query_logger::QueryLogger;
use nosql_orm::logging::wrapper::ProviderWrapper;
use nosql_orm::logging::LoggingStrategy;
use nosql_orm::provider::Storage;
use nosql_orm::providers::json::JsonProvider;
use std::sync::Arc;

async fn create_test_provider() -> (JsonProvider, tempfile::TempDir) {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  (provider, temp_dir)
}

#[tokio::test]
async fn test_query_logger_enable_disable() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = QueryLogger::new(provider);

  assert!(logger.is_enabled().await);

  logger.disable().await;
  assert!(!logger.is_enabled().await);

  logger.enable().await;
  assert!(logger.is_enabled().await);
}

#[tokio::test]
async fn test_query_logger_new() {
  let (provider, _tmp) = create_test_provider().await;
  let _logger = QueryLogger::new(provider);
}

#[tokio::test]
async fn test_query_logger_insert_logs() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = QueryLogger::new(provider);

  let doc = serde_json::json!({"name": "test"});
  let _ = logger.insert("users", doc).await;
}

#[tokio::test]
async fn test_query_logger_find_by_id() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = QueryLogger::new(provider.clone());

  let doc = serde_json::json!({"name": "test"});
  provider.insert("users", doc).await.unwrap();

  let _ = logger.find_by_id("users", "nonexistent").await;
}

#[tokio::test]
async fn test_query_logger_find_many() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = QueryLogger::new(provider);

  let _ = logger
    .find_many("users", None, None, None, None, true)
    .await;
}

#[tokio::test]
async fn test_query_logger_update() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = QueryLogger::new(provider.clone());

  let doc = serde_json::json!({"name": "test"});
  let inserted = provider.insert("users", doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let _ = logger
    .update("users", id, serde_json::json!({"name": "updated"}))
    .await;
}

#[tokio::test]
async fn test_query_logger_delete() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = QueryLogger::new(provider.clone());

  let doc = serde_json::json!({"name": "test"});
  let inserted = provider.insert("users", doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let _ = logger.delete("users", id).await;
}

#[tokio::test]
async fn test_query_logger_find_all() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = QueryLogger::new(provider);

  let _ = logger.find_all("users").await;
}

#[tokio::test]
async fn test_query_logger_count() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = QueryLogger::new(provider);

  let _ = logger.count("users", None).await;
}

#[tokio::test]
async fn test_query_logger_exists() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = QueryLogger::new(provider);

  let _ = logger.exists("users", "nonexistent").await;
}

#[tokio::test]
async fn test_provider_wrapper_insert() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider.clone()), log_dir.path().to_path_buf());
  let wrapper = ProviderWrapper::new(provider, logger);

  let doc = serde_json::json!({"name": "test"});
  let result = wrapper.insert("users", doc).await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_find_by_id() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider.clone()), log_dir.path().to_path_buf());
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
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider.clone()), log_dir.path().to_path_buf());
  let wrapper = ProviderWrapper::new(provider, logger);

  let result = wrapper
    .find_many("users", None, None, None, None, true)
    .await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_update() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider.clone()), log_dir.path().to_path_buf());
  let wrapper = ProviderWrapper::new(provider, logger);

  let doc = serde_json::json!({"name": "test"});
  let inserted = wrapper.insert("users", doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let result = wrapper
    .update("users", id, serde_json::json!({"name": "updated"}))
    .await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_delete() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider.clone()), log_dir.path().to_path_buf());
  let wrapper = ProviderWrapper::new(provider, logger);

  let doc = serde_json::json!({"name": "test"});
  let inserted = wrapper.insert("users", doc).await.unwrap();
  let id = inserted["id"].as_str().unwrap();

  let result = wrapper.delete("users", id).await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_count() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider.clone()), log_dir.path().to_path_buf());
  let wrapper = ProviderWrapper::new(provider, logger);

  let result = wrapper.count("users", None).await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_provider_wrapper_find_all() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider.clone()), log_dir.path().to_path_buf());
  let wrapper = ProviderWrapper::new(provider, logger);

  let result = wrapper.find_all("users").await;
  assert!(result.is_ok());
}

#[tokio::test]
async fn test_file_query_logger_new() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let _logger = FileQueryLogger::new(Arc::new(provider), log_dir.path().to_path_buf());
}

#[tokio::test]
async fn test_file_query_logger_enable_disable() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider), log_dir.path().to_path_buf());

  assert!(logger.is_enabled().await);

  logger.disable().await;
  assert!(!logger.is_enabled().await);

  logger.enable().await;
  assert!(logger.is_enabled().await);
}

#[tokio::test]
async fn test_file_query_logger_log() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider), log_dir.path().to_path_buf());

  logger.log_start("INSERT", "users").await;
  logger.log_complete("INSERT", "users", 10, true).await;

  let logs = logger.get_recent_logs(10).await;
  assert!(!logs.is_empty(), "expected file logger to record entries");
  assert!(
    logs
      .iter()
      .any(|l| l.contains("INSERT") && l.contains("users")),
    "expected log entry mentioning the INSERT operation on 'users', got: {:?}",
    logs
  );
}

#[tokio::test]
async fn test_db_query_logger_new() {
  let (provider, _tmp) = create_test_provider().await;
  let _logger = DbQueryLogger::new(provider);
}

#[tokio::test]
async fn test_db_query_logger_enable_disable() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = DbQueryLogger::new(provider);

  assert!(logger.is_enabled().await);

  logger.disable().await;
  assert!(!logger.is_enabled().await);

  logger.enable().await;
  assert!(logger.is_enabled().await);
}

#[tokio::test]
async fn test_db_query_logger_log() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = DbQueryLogger::new(provider.clone());

  logger.log_start("INSERT", "users").await;
  logger.log_complete("INSERT", "users", 10, true).await;

  let logged = provider.count("query_logs", None).await.unwrap();
  assert!(
    logged >= 1,
    "expected the db logger to persist at least one entry, got {}",
    logged
  );
}

#[test]
fn test_logging_mod_exports() {
  use nosql_orm::logging;
  let _ = std::any::type_name::<logging::DbQueryLogger>();
  let _ = std::any::type_name::<logging::FileQueryLogger<JsonProvider>>();
  let _ = std::any::type_name::<logging::QueryLogger<JsonProvider>>();
  let _ = std::any::type_name::<logging::ProviderWrapper<JsonProvider, DbQueryLogger>>();
}

async fn exercise_logging_strategy<S: LoggingStrategy>(logger: &S) {
  logger.log_start("SELECT", "users").await;
  logger.log_complete("SELECT", "users", 1, true).await;
  logger.log_error("SELECT", "users", "boom").await;
}

#[tokio::test]
async fn test_logging_strategy_generic() {
  let (provider, _tmp) = create_test_provider().await;
  let logger = DbQueryLogger::new(provider);
  exercise_logging_strategy(&logger).await;

  let (provider2, _tmp2) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let file_logger = FileQueryLogger::new(Arc::new(provider2), log_dir.path().to_path_buf());
  exercise_logging_strategy(&file_logger).await;
}

#[tokio::test]
async fn test_logging_strategy_clone() {
  let (provider, _tmp) = create_test_provider().await;
  let log_dir = tempfile::TempDir::new().unwrap();
  let logger = FileQueryLogger::new(Arc::new(provider), log_dir.path().to_path_buf());
  let _cloned = logger.clone();
}
