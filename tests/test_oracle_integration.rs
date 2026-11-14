// Oracle integration tests.
//
// Every test is `#[ignore]`d: they need a database AND the Oracle Client
// library (ODPI-C loads it at connect time), e.g.
//
//   docker run -d -p 1521:1521 gvenzl/oracle-free:23-slim
//
// and then:
//
//   ORACLE_DSN='system/oracle@//localhost:1521/XEPDB1' \
//     cargo test --features sql-oracle -- --ignored

use nosql_orm::error::OrmResult;
use nosql_orm::provider::{AdminCommands, SchemaIntrospection, Storage, TransactionControl};
use nosql_orm::providers::sql::OracleProvider;
use nosql_orm::query::Filter;
use serde_json::{json, Value};

// Oracle upper-cases an unquoted identifier while every statement the ORM
// builds quotes them, so the DDL quotes both the table and its columns to
// stay in the same case space as the reads.
const TABLE_DDL: &str = "CREATE TABLE \"test_notes\" (\"id\" VARCHAR2(400) PRIMARY KEY, \"title\" VARCHAR2(400), \"priority\" NUMBER(10), \"completed\" NUMBER(1))";

fn dsn() -> String {
  std::env::var("ORACLE_DSN")
    .unwrap_or_else(|_| "system/oracle@//localhost:1521/XEPDB1".to_string())
}

async fn fresh_provider() -> OrmResult<OracleProvider> {
  let provider = OracleProvider::connect(dsn()).await?;
  let _ = AdminCommands::execute_raw(&provider, "DROP TABLE \"test_notes\"", vec![]).await;
  AdminCommands::execute_raw(&provider, TABLE_DDL, vec![]).await?;
  Ok(provider)
}

#[tokio::test]
#[ignore = "needs an Oracle database and the Oracle Client library"]
async fn health_check_reports_the_server_version() -> OrmResult<()> {
  let provider = OracleProvider::connect(dsn()).await?;
  let health = provider.health_check_detailed().await?;
  assert!(health.healthy);
  let version = health.server_version.unwrap_or_default();
  assert!(version.to_uppercase().contains("ORACLE"), "{}", version);
  Ok(())
}

#[tokio::test]
#[ignore = "needs an Oracle database and the Oracle Client library"]
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
  // A NUMBER(10) comes back as a JSON number, not as text.
  assert_eq!(found.get("priority").and_then(Value::as_i64), Some(1));

  assert_eq!(provider.count("test_notes", None).await?, 1);
  Ok(())
}

#[tokio::test]
#[ignore = "needs an Oracle database and the Oracle Client library"]
async fn find_many_filters_and_limits() -> OrmResult<()> {
  let provider = fresh_provider().await?;
  for priority in 1..=5 {
    provider
      .insert(
        "test_notes",
        json!({"title": format!("note-{}", priority), "priority": priority}),
      )
      .await?;
  }

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

  let page = provider
    .find_many("test_notes", None, Some(2), Some(2), None, true)
    .await?;
  assert_eq!(page.len(), 2);

  Ok(())
}

#[tokio::test]
#[ignore = "needs an Oracle database and the Oracle Client library"]
async fn update_and_delete_outside_a_transaction_commit_themselves() -> OrmResult<()> {
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

  let removed = provider.delete("test_notes", &id).await?;
  assert!(removed);
  assert!(provider.find_by_id("test_notes", &id).await?.is_none());
  Ok(())
}

#[tokio::test]
#[ignore = "needs an Oracle database and the Oracle Client library"]
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
  provider.rollback_transaction(tx).await?;
  assert_eq!(provider.count("test_notes", None).await?, 1);
  assert!(!provider.is_transaction_active().await?);

  Ok(())
}

#[tokio::test]
#[ignore = "needs an Oracle database and the Oracle Client library"]
async fn create_and_drop_a_table_through_the_admin_commands() -> OrmResult<()> {
  let provider = OracleProvider::connect(dsn()).await?;
  let _ = AdminCommands::drop_collection(&provider, "notes_admin").await;

  // The shared DDL is translated for Oracle: TEXT becomes VARCHAR2 and the
  // key column keeps the quoting the reads use.
  provider.create_collection("notes_admin", None).await?;

  let collections = provider.list_collections().await?;
  assert!(collections.iter().any(|meta| meta.name == "notes_admin"));

  let schema = provider.describe_collection("notes_admin").await?;
  assert!(schema.fields.contains_key("id"));

  provider.drop_collection("notes_admin").await?;
  let after = provider.list_collections().await?;
  assert!(!after.iter().any(|meta| meta.name == "notes_admin"));
  Ok(())
}

#[tokio::test]
#[ignore = "needs an Oracle database and the Oracle Client library"]
async fn schema_introspection_reads_the_data_dictionary() -> OrmResult<()> {
  let provider = fresh_provider().await?;

  let schema = provider.describe_collection("test_notes").await?;
  assert!(schema.fields.contains_key("title"));
  assert!(schema.fields.contains_key("priority"));

  let database = provider.get_database_name().await?;
  assert!(!database.is_empty());
  let databases = provider.list_databases().await?;
  assert_eq!(databases, vec![database]);

  Ok(())
}
