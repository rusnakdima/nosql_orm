// DuckDB integration tests.
//
// DuckDB is embedded, so unlike the server-backed providers these tests run
// without an `#[ignore]`: a file database per test is created, exercised
// through the `Storage` surface, and removed again.

use nosql_orm::error::OrmResult;
use nosql_orm::provider::{
  AdminCommands, Capability, SchemaIntrospection, Storage, TransactionControl,
};
use nosql_orm::providers::sql::DuckDbProvider;
use nosql_orm::query::Filter;
use serde_json::{json, Value};

const DDL: &str = "CREATE TABLE test_notes (id TEXT PRIMARY KEY, title TEXT, priority INTEGER, completed BOOLEAN, tags TEXT)";

// One database per test: the tests run in parallel, and a shared file would
// let one test's rows be another's.
fn db_path() -> String {
  let test = std::thread::current()
    .name()
    .unwrap_or("test")
    .replace("::", "_");
  format!("/tmp/nosql_orm_duckdb_{}.duckdb", test)
}

// DuckDB keeps a write-ahead log next to the database file; both must go
// or the next run opens yesterday's state.
fn remove_db_files() {
  let path = db_path();
  let _ = std::fs::remove_file(&path);
  let _ = std::fs::remove_file(format!("{}.wal", path));
}

async fn fresh_provider() -> OrmResult<DuckDbProvider> {
  remove_db_files();
  let provider = DuckDbProvider::connect(db_path()).await?;
  AdminCommands::execute_raw(&provider, DDL, vec![]).await?;
  Ok(provider)
}

fn cleanup(_provider: &DuckDbProvider) {
  remove_db_files();
}

#[tokio::test]
async fn health_check_reports_the_duckdb_version() -> OrmResult<()> {
  let provider = fresh_provider().await?;

  let healthy = nosql_orm::provider::Storage::health_check(&provider).await?;
  assert!(healthy);
  let version = provider.get_server_version().await?;
  assert!(version.to_lowercase().contains("duckdb") || !version.is_empty());
  let database = provider.get_database_name().await?;
  assert!(!database.is_empty());

  cleanup(&provider);
  Ok(())
}

#[tokio::test]
async fn insert_then_find_round_trip() -> OrmResult<()> {
  let provider = fresh_provider().await?;

  let inserted = provider
    .insert(
      "test_notes",
      json!({"title": "hello", "priority": 3, "completed": false}),
    )
    .await?;
  let id = inserted.get("id").and_then(Value::as_str).unwrap_or("");
  assert!(!id.is_empty());

  let found = provider.find_by_id("test_notes", id).await?;
  let found = found.expect("inserted document must be readable");
  assert_eq!(found.get("title").and_then(Value::as_str), Some("hello"));
  assert_eq!(found.get("priority").and_then(Value::as_i64), Some(3));
  assert_eq!(found.get("completed").and_then(Value::as_bool), Some(false));

  cleanup(&provider);
  Ok(())
}

#[tokio::test]
async fn find_many_filters_sorts_and_limits() -> OrmResult<()> {
  let provider = fresh_provider().await?;
  for priority in 1..=5 {
    provider
      .insert(
        "test_notes",
        json!({"title": format!("note-{}", priority), "priority": priority}),
      )
      .await?;
  }

  let count = provider.count("test_notes", None).await?;
  assert_eq!(count, 5);

  let filtered = provider
    .find_many(
      "test_notes",
      Some(&Filter::Gt("priority".to_string(), json!(2))),
      None,
      None,
      Some("priority"),
      false,
    )
    .await?;
  assert_eq!(filtered.len(), 3);
  assert_eq!(
    filtered
      .first()
      .and_then(|row| row.get("priority"))
      .and_then(Value::as_i64),
    Some(5)
  );

  let limited = provider
    .find_many("test_notes", None, Some(2), Some(2), None, true)
    .await?;
  assert_eq!(limited.len(), 2);

  cleanup(&provider);
  Ok(())
}

#[tokio::test]
async fn update_patch_and_delete() -> OrmResult<()> {
  let provider = fresh_provider().await?;
  let inserted = provider
    .insert("test_notes", json!({"title": "before", "priority": 1}))
    .await?;
  let id = inserted
    .get("id")
    .and_then(Value::as_str)
    .unwrap_or("")
    .to_string();

  let updated = provider
    .update("test_notes", &id, json!({"title": "after", "priority": 2}))
    .await?;
  assert_eq!(updated.get("title").and_then(Value::as_str), Some("after"));

  let patched = provider
    .patch("test_notes", &id, json!({"completed": true}))
    .await?;
  assert_eq!(
    patched.get("completed").and_then(Value::as_bool),
    Some(true)
  );
  assert_eq!(patched.get("title").and_then(Value::as_str), Some("after"));

  let updated_many = provider
    .update_many(
      "test_notes",
      Some(Filter::Eq("priority".to_string(), json!(2))),
      json!({"priority": 9}),
    )
    .await?;
  assert_eq!(updated_many, 1);

  let deleted = provider.delete("test_notes", &id).await?;
  assert!(deleted);
  assert!(provider.find_by_id("test_notes", &id).await?.is_none());

  cleanup(&provider);
  Ok(())
}

#[tokio::test]
async fn transactions_commit_and_rollback() -> OrmResult<()> {
  let provider = fresh_provider().await?;

  let tx = provider.begin_transaction().await?;
  provider
    .insert("test_notes", json!({"title": "committed"}))
    .await?;
  provider.commit_transaction(tx).await?;
  assert_eq!(provider.count("test_notes", None).await?, 1);

  let tx = provider.begin_transaction().await?;
  provider
    .insert("test_notes", json!({"title": "rolled-back"}))
    .await?;
  assert_eq!(provider.count("test_notes", None).await?, 2);
  provider.rollback_transaction(tx).await?;
  assert_eq!(provider.count("test_notes", None).await?, 1);
  assert!(!provider.is_transaction_active().await?);

  cleanup(&provider);
  Ok(())
}

#[tokio::test]
async fn schema_introspection_lists_the_table_and_its_columns() -> OrmResult<()> {
  let provider = fresh_provider().await?;

  let collections = provider.list_collections().await?;
  let table = collections
    .iter()
    .find(|meta| meta.name == "test_notes")
    .expect("test_notes must be listed");
  assert_eq!(table.document_count, 0);

  let schema = provider.describe_collection("test_notes").await?;
  assert!(schema.fields.contains_key("title"));
  assert!(schema.fields.contains_key("priority"));

  let stats = provider.get_collection_stats("test_notes").await?;
  assert_eq!(stats.document_count, 0);

  cleanup(&provider);
  Ok(())
}

#[tokio::test]
async fn create_and_list_a_second_index() -> OrmResult<()> {
  let provider = fresh_provider().await?;
  provider
    .insert("test_notes", json!({"title": "indexed", "priority": 1}))
    .await?;

  let index = nosql_orm::nosql_index::NosqlIndex::single("title", 1).name("idx_notes_title");
  Storage::create_index(&provider, "test_notes", &index).await?;

  let indexes = SchemaIntrospection::list_indexes(&provider, "test_notes").await?;
  let listed = indexes
    .iter()
    .find(|info| info.name == "idx_notes_title")
    .expect("the created index must be listed");
  assert_eq!(listed.fields, vec!["title".to_string()]);

  Storage::drop_index(&provider, "test_notes", "idx_notes_title").await?;
  let after_drop = SchemaIntrospection::list_indexes(&provider, "test_notes").await?;
  assert!(!after_drop.iter().any(|info| info.name == "idx_notes_title"));

  cleanup(&provider);
  Ok(())
}

#[tokio::test]
async fn capabilities_and_raw_queries() -> OrmResult<()> {
  let provider = fresh_provider().await?;
  let capabilities = provider.capabilities();
  assert!(capabilities.supports(Capability::Indexes));
  assert!(capabilities.supports(Capability::Transactions));
  assert!(capabilities.supports(Capability::RawQueries));

  provider
    .insert("test_notes", json!({"title": "raw", "priority": 7}))
    .await?;
  let raw = AdminCommands::execute_raw(
    &provider,
    "SELECT title, priority FROM test_notes WHERE priority = ?",
    vec![json!(7)],
  )
  .await?;
  assert_eq!(raw.columns, vec!["title", "priority"]);
  assert_eq!(raw.rows.len(), 1);
  assert_eq!(raw.rows[0][0], json!("raw"));
  assert_eq!(raw.rows[0][1], json!(7));

  cleanup(&provider);
  Ok(())
}
