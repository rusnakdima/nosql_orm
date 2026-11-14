// ClickHouse integration tests.
//
// Every test is `#[ignore]`d: they need a running server, e.g.
//
//   docker run -p 8123:8123 clickhouse/clickhouse-server
//
// and then:
//
//   cargo test --features sql-clickhouse -- --ignored
//
// `CLICKHOUSE_URL` overrides the default `http://localhost:8123`.

use nosql_orm::error::{OrmError, OrmResult};
use nosql_orm::provider::{AdminCommands, SchemaIntrospection, Storage, TransactionControl};
use nosql_orm::providers::sql::ClickHouseProvider;
use nosql_orm::query::Filter;
use serde_json::{json, Value};

const TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS test_notes (id String, title String, priority Int32, completed UInt8) ENGINE = MergeTree ORDER BY id";

fn url() -> String {
  std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".to_string())
}

async fn fresh_provider() -> OrmResult<ClickHouseProvider> {
  let provider = ClickHouseProvider::connect(url()).await?;
  let _ = AdminCommands::execute_raw(&provider, "DROP TABLE IF EXISTS test_notes", vec![]).await;
  AdminCommands::execute_raw(&provider, TABLE_DDL, vec![]).await?;
  Ok(provider)
}

#[tokio::test]
#[ignore = "needs a running ClickHouse server"]
async fn health_check_reports_the_server_version() -> OrmResult<()> {
  let provider = ClickHouseProvider::connect(url()).await?;
  let health = provider.health_check_detailed().await?;
  assert!(health.healthy);
  let version = health.server_version.unwrap_or_default();
  assert!(version.to_lowercase().contains("clickhouse"), "{}", version);
  Ok(())
}

#[tokio::test]
#[ignore = "needs a running ClickHouse server"]
async fn insert_find_count_round_trip() -> OrmResult<()> {
  let provider = fresh_provider().await?;

  let inserted = provider
    .insert(
      "test_notes",
      json!({"title": "first", "priority": 1, "completed": false}),
    )
    .await?;
  let id = inserted.get("id").and_then(Value::as_str).unwrap_or("");
  assert!(!id.is_empty());

  let found = provider
    .find_by_id("test_notes", id)
    .await?
    .expect("inserted document must be readable");
  assert_eq!(found.get("title").and_then(Value::as_str), Some("first"));

  assert_eq!(provider.count("test_notes", None).await?, 1);
  let filtered = provider
    .find_many(
      "test_notes",
      Some(&Filter::Gt("priority".to_string(), json!(0))),
      None,
      None,
      None,
      true,
    )
    .await?;
  assert_eq!(filtered.len(), 1);

  Ok(())
}

#[tokio::test]
#[ignore = "needs a running ClickHouse server"]
async fn delete_counts_the_rows_it_matched() -> OrmResult<()> {
  let provider = fresh_provider().await?;
  for priority in 1..=3 {
    provider
      .insert(
        "test_notes",
        json!({"title": format!("note-{}", priority), "priority": priority}),
      )
      .await?;
  }

  let removed = provider
    .delete_many(
      "test_notes",
      Some(Filter::Gt("priority".to_string(), json!(1))),
    )
    .await?;
  assert_eq!(removed, 2);
  assert_eq!(provider.count("test_notes", None).await?, 1);
  Ok(())
}

#[tokio::test]
#[ignore = "needs a running ClickHouse server"]
async fn update_many_runs_as_a_mutation() -> OrmResult<()> {
  let provider = fresh_provider().await?;
  for priority in 1..=2 {
    provider
      .insert(
        "test_notes",
        json!({"title": format!("note-{}", priority), "priority": priority}),
      )
      .await?;
  }

  let updated = provider
    .update_many("test_notes", None, json!({"priority": 9}))
    .await?;
  assert_eq!(updated, 2);

  let rows = provider
    .find_many("test_notes", None, None, None, None, true)
    .await?;
  assert!(rows
    .iter()
    .all(|row| row.get("priority") == Some(&json!(9))));
  Ok(())
}

#[tokio::test]
#[ignore = "needs a running ClickHouse server"]
async fn schema_introspection_reads_the_system_tables() -> OrmResult<()> {
  let provider = fresh_provider().await?;

  let collections = provider.list_collections().await?;
  assert!(collections.iter().any(|meta| meta.name == "test_notes"));

  let schema = provider.describe_collection("test_notes").await?;
  assert!(schema.fields.contains_key("title"));

  let stats = provider.get_collection_stats("test_notes").await?;
  assert_eq!(stats.document_count, 0);

  let indexes = SchemaIntrospection::list_indexes(&provider, "test_notes").await?;
  assert!(indexes.is_empty());

  Ok(())
}

#[tokio::test]
#[ignore = "needs a running ClickHouse server"]
async fn create_and_drop_a_table_through_the_admin_commands() -> OrmResult<()> {
  let provider = ClickHouseProvider::connect(url()).await?;
  let _ = AdminCommands::drop_collection(&provider, "notes_admin").await;

  provider.create_collection("notes_admin", None).await?;
  provider
    .insert("notes_admin", json!({"title": "created"}))
    .await?;
  assert_eq!(provider.count("notes_admin", None).await?, 1);

  provider.drop_collection("notes_admin").await?;
  assert!(provider.find_by_id("notes_admin", "missing").await.is_err());
  Ok(())
}

#[tokio::test]
#[ignore = "needs a running ClickHouse server"]
async fn transactions_are_reported_as_unsupported_rather_than_faked() -> OrmResult<()> {
  let provider = ClickHouseProvider::connect(url()).await?;
  let result = provider.begin_transaction().await;
  assert!(matches!(result, Err(OrmError::NotSupported(_))));
  assert!(!provider.is_transaction_active().await?);
  Ok(())
}
