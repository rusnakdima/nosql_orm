use nosql_orm::error::OrmResult;
use nosql_orm::migrations::migration::{Migration, MigrationMeta, SqlMigration};
use nosql_orm::migrations::runner::MigrationRunner;
use nosql_orm::provider::Storage;
use nosql_orm::providers::json::JsonProvider;

async fn create_runner() -> (MigrationRunner<JsonProvider>, tempfile::TempDir) {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let runner = MigrationRunner::new(provider);
  (runner, temp_dir)
}

#[tokio::test]
async fn test_migration_runner_new() {
  let (runner, _temp_dir) = create_runner().await;
  let _ = runner;
}

#[tokio::test]
async fn test_migration_runner_add_migration() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let mut runner = MigrationRunner::new(provider);

  let migration = SqlMigration::new(
    1,
    "test_migration",
    "CREATE TABLE users (id TEXT PRIMARY KEY, name TEXT);",
    "DROP TABLE users;",
  );

  runner.add_migration(migration);
}

struct TestMigration {
  version_val: i64,
  name_val: String,
  up_called: std::sync::Arc<std::sync::atomic::AtomicBool>,
  down_called: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[tokio::test]
async fn test_custom_migration_impl() {
  let (mut runner, _temp_dir) = create_runner().await;
  let migration = TestMigration::new(7, "custom");
  let up_called = migration.up_called.clone();
  let down_called = migration.down_called.clone();

  assert_eq!(migration.version(), 7);
  assert_eq!(migration.name(), "custom");

  runner.add_migration(migration);
  let results = runner.run_all_pending().await.unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0].version, 7);
  assert_eq!(results[0].name, "custom");
  assert!(up_called.load(std::sync::atomic::Ordering::Relaxed));
  assert!(!down_called.load(std::sync::atomic::Ordering::Relaxed));

  runner.rollback(1).await.unwrap();
  assert!(down_called.load(std::sync::atomic::Ordering::Relaxed));
}

impl TestMigration {
  fn new(version: i64, name: &str) -> Self {
    Self {
      version_val: version,
      name_val: name.to_string(),
      up_called: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
      down_called: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    }
  }
}

#[async_trait::async_trait]
impl Migration<JsonProvider> for TestMigration {
  fn version(&self) -> i64 {
    self.version_val
  }

  fn name(&self) -> &str {
    &self.name_val
  }

  async fn up(&self, _provider: &JsonProvider) -> OrmResult<()> {
    self
      .up_called
      .store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
  }

  async fn down(&self, _provider: &JsonProvider) -> OrmResult<()> {
    self
      .down_called
      .store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
  }
}

#[tokio::test]
async fn test_migration_meta_serialization() {
  let meta = MigrationMeta {
    version: 1,
    name: "test".to_string(),
    applied_at: Some(chrono::Utc::now()),
  };

  let json = serde_json::to_string(&meta).unwrap();
  let parsed: MigrationMeta = serde_json::from_str(&json).unwrap();

  assert_eq!(parsed.version, 1);
  assert_eq!(parsed.name, "test");
}

#[tokio::test]
async fn test_migration_meta_debug() {
  let meta = MigrationMeta {
    version: 1,
    name: "test".to_string(),
    applied_at: None,
  };

  let debug_str = format!("{:?}", meta);
  assert!(debug_str.contains("MigrationMeta"));
  assert!(debug_str.contains("1"));
  assert!(debug_str.contains("test"));
}

#[tokio::test]
async fn test_sql_migration_new() {
  let migration = SqlMigration::new(
    1,
    "create_users",
    "CREATE TABLE users (id TEXT PRIMARY KEY);",
    "DROP TABLE users;",
  );

  assert_eq!(migration.version, 1);
  assert_eq!(migration.name, "create_users");
  assert_eq!(
    migration.up_sql,
    "CREATE TABLE users (id TEXT PRIMARY KEY);"
  );
  assert_eq!(migration.down_sql, "DROP TABLE users;");
}

#[tokio::test]
async fn test_sql_migration_trait() {
  let migration = SqlMigration::new(1, "test", "SELECT 1;", "SELECT 0;");

  assert_eq!(
    <SqlMigration as Migration<JsonProvider>>::version(&migration),
    1
  );
  assert_eq!(
    <SqlMigration as Migration<JsonProvider>>::name(&migration),
    "test"
  );
}

#[tokio::test]
async fn test_sql_migration_up_sql() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();

  let migration = SqlMigration::new(
    1,
    "insert_test",
    "INSERT INTO test (id, data) VALUES ('1', 'test');",
    "DELETE FROM test WHERE id = '1';",
  );

  migration.up(&provider).await.unwrap();
}

#[tokio::test]
async fn test_sql_migration_down_sql() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();

  let migration = SqlMigration::new(
    1,
    "insert_test",
    "INSERT INTO test (id, data) VALUES ('1', 'test');",
    "DELETE FROM test WHERE id = '1';",
  );

  migration.up(&provider).await.unwrap();
  migration.down(&provider).await.unwrap();
}

#[tokio::test]
async fn test_run_all_pending_empty() {
  let (runner, _temp_dir) = create_runner().await;
  let results = runner.run_all_pending().await.unwrap();
  assert!(results.is_empty());
}

#[tokio::test]
async fn test_run_all_pending_with_migrations() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let mut runner = MigrationRunner::new(provider);

  runner.add_migration(SqlMigration::new(
    1,
    "create_users",
    "CREATE TABLE users (id TEXT PRIMARY KEY, name TEXT);",
    "DROP TABLE users;",
  ));

  runner.add_migration(SqlMigration::new(
    2,
    "create_posts",
    "CREATE TABLE posts (id TEXT PRIMARY KEY, title TEXT);",
    "DROP TABLE posts;",
  ));

  let results = runner.run_all_pending().await.unwrap();
  assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_run_all_pending_skips_already_applied() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let mut runner = MigrationRunner::new(provider.clone());

  runner.add_migration(SqlMigration::new(
    1,
    "create_users",
    "CREATE TABLE users (id TEXT PRIMARY KEY);",
    "DROP TABLE users;",
  ));

  runner.run_all_pending().await.unwrap();
  runner.run_all_pending().await.unwrap();
  runner.run_all_pending().await.unwrap();

  let docs = provider.find_all("_migrations").await.unwrap();
  assert_eq!(docs.len(), 2);
}

#[tokio::test]
async fn test_migration_meta_clone() {
  let meta = MigrationMeta {
    version: 1,
    name: "test".to_string(),
    applied_at: Some(chrono::Utc::now()),
  };

  let cloned = meta.clone();
  assert_eq!(cloned.version, meta.version);
  assert_eq!(cloned.name, meta.name);
}
