use nosql_orm::error::OrmResult;
use nosql_orm::migrations::migration::{JsonMigration, Migration, MigrationMeta, SqlMigration};
use nosql_orm::migrations::runner::MigrationRunner;
use nosql_orm::provider::Storage;
use nosql_orm::providers::json::JsonProvider;
use serde_json::json;

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
async fn test_run_all_pending_empty() {
  let (runner, _temp_dir) = create_runner().await;
  let results = runner.run_all_pending().await.unwrap();
  assert!(results.is_empty());
}

#[tokio::test]
async fn test_run_all_pending_with_migrations() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let mut runner = MigrationRunner::new(provider.clone());

  // JsonMigration rather than SqlMigration: the JSON provider has no query
  // language, so a SQL body would honestly answer `NotSupported` here.
  // SqlMigration execution is covered by tests/test_migration_sql.rs.
  runner.add_migration(JsonMigration::new(
    1,
    "create_settings",
    json!([{"op": "insert", "collection": "settings",
             "doc": {"id": "app", "theme": "dark"}}]),
    json!([{"op": "delete", "collection": "settings", "id": "app"}]),
  ));

  runner.add_migration(JsonMigration::new(
    2,
    "add_email",
    json!([{"op": "insert", "collection": "contacts",
             "doc": {"id": "c1", "email": ""}}]),
    json!([{"op": "delete", "collection": "contacts", "id": "c1"}]),
  ));

  let results = runner.run_all_pending().await.unwrap();
  assert_eq!(results.len(), 2);

  // The bodies must really have run — this is the whole point of the fix.
  assert_eq!(provider.count("settings", None).await.unwrap(), 1);
  assert_eq!(provider.count("contacts", None).await.unwrap(), 1);
}

#[tokio::test]
async fn test_run_all_pending_skips_already_applied() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let mut runner = MigrationRunner::new(provider.clone());

  runner.add_migration(JsonMigration::new(
    1,
    "create_settings",
    json!([{"op": "insert", "collection": "settings",
             "doc": {"id": "app", "theme": "dark"}}]),
    json!([{"op": "delete", "collection": "settings", "id": "app"}]),
  ));

  runner.run_all_pending().await.unwrap();
  runner.run_all_pending().await.unwrap();
  runner.run_all_pending().await.unwrap();

  // One body execution, so a duplicate insert would have failed loudly.
  assert_eq!(provider.count("settings", None).await.unwrap(), 1);

  // `_schema` bookkeeping row + one migration record.
  let docs = provider.find_all("_migrations").await.unwrap();
  assert_eq!(docs.len(), 2);
}

// Rollback must undo the body *and* delete the applied record, otherwise the
// migration stays "applied" forever and can never run again.
#[tokio::test]
async fn test_rollback_removes_the_record_so_the_migration_can_rerun() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let mut runner = MigrationRunner::new(provider.clone());

  runner.add_migration(JsonMigration::new(
    1,
    "create_settings",
    json!([{"op": "insert", "collection": "settings",
             "doc": {"id": "app", "theme": "dark"}}]),
    json!([{"op": "delete", "collection": "settings", "id": "app"}]),
  ));

  runner.run_all_pending().await.unwrap();
  assert_eq!(provider.count("settings", None).await.unwrap(), 1);
  assert_eq!(runner.status().await.unwrap().len(), 1);

  runner.rollback(1).await.unwrap();
  assert_eq!(
    provider.count("settings", None).await.unwrap(),
    0,
    "rollback must run the down script"
  );
  assert_eq!(
    runner.status().await.unwrap().len(),
    0,
    "rollback must delete the applied record"
  );

  let reapplied = runner.run_all_pending().await.unwrap();
  assert_eq!(reapplied.len(), 1, "a rolled-back migration must re-run");
  assert_eq!(provider.count("settings", None).await.unwrap(), 1);
}

// The `_schema` bookkeeping row is not a migration: it must not show up in
// `status()` as a phantom version 0.
#[tokio::test]
async fn test_status_excludes_the_schema_marker() {
  let (runner, _temp_dir) = create_runner().await;
  let status = runner.status().await.unwrap();
  assert!(
    status.is_empty(),
    "status must not report the _schema marker, got {:?}",
    status
  );
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
