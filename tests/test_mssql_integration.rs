// SQL Server integration tests.
//
// Every test is `#[ignore]`d: they need a running server, e.g.
//
//   docker run -e ACCEPT_EULA=Y -e MSSQL_SA_PASSWORD=Your_password123 \
//     -p 1433:1433 mcr.microsoft.com/mssql/server:2022-latest
//
// and then:
//
//   MSSQL_DSN='server=tcp:localhost,1433;user id=sa;password=Your_password123;TrustServerCertificate=true' \
//     cargo test --features sql-mssql -- --ignored
//
// The driver negotiates TLS (native-tls), so a local container needs
// `TrustServerCertificate=true` or a certificate the host trusts.

use nosql_orm::error::{OrmError, OrmResult};
use nosql_orm::provider::{AdminCommands, SchemaIntrospection, Storage, TransactionControl};
use nosql_orm::providers::sql::MssqlProvider;
use nosql_orm::query::Filter;
use serde_json::{json, Value};

const TABLE_DDL: &str =
  "CREATE TABLE test_notes (id NVARCHAR(450) PRIMARY KEY, title NVARCHAR(400), priority INT, completed BIT)";

fn dsn() -> String {
  std::env::var("MSSQL_DSN").unwrap_or_else(|_| {
    "server=tcp:localhost,1433;user id=sa;password=Your_password123;TrustServerCertificate=true"
      .to_string()
  })
}

async fn fresh_provider() -> OrmResult<MssqlProvider> {
  let provider = MssqlProvider::connect(dsn()).await?;
  let _ = AdminCommands::execute_raw(&provider, "DROP TABLE IF EXISTS test_notes", vec![]).await;
  AdminCommands::execute_raw(&provider, TABLE_DDL, vec![]).await?;
  Ok(provider)
}

#[tokio::test]
#[ignore = "needs a running SQL Server instance"]
async fn health_check_reports_the_server_version() -> OrmResult<()> {
  let provider = MssqlProvider::connect(dsn()).await?;
  let health = provider.health_check_detailed().await?;
  assert!(health.healthy);
  let version = health.server_version.unwrap_or_default();
  assert!(version.to_uppercase().contains("SQL SERVER") || !version.is_empty());
  Ok(())
}

#[tokio::test]
#[ignore = "needs a running SQL Server instance"]
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
  assert_eq!(found.get("priority").and_then(Value::as_i64), Some(1));

  assert_eq!(provider.count("test_notes", None).await?, 1);
  Ok(())
}

#[tokio::test]
#[ignore = "needs a running SQL Server instance"]
async fn find_many_translates_limit_and_offset_into_fetch() -> OrmResult<()> {
  let provider = fresh_provider().await?;
  for priority in 1..=5 {
    provider
      .insert(
        "test_notes",
        json!({"title": format!("note-{}", priority), "priority": priority}),
      )
      .await?;
  }

  // No ORDER BY: the rewrite synthesizes one, as T-SQL requires for FETCH.
  let page = provider
    .find_many("test_notes", None, Some(2), Some(2), None, true)
    .await?;
  assert_eq!(page.len(), 2);

  let sorted = provider
    .find_many(
      "test_notes",
      Some(&Filter::Gte("priority".to_string(), json!(4))),
      None,
      None,
      Some("priority"),
      false,
    )
    .await?;
  assert_eq!(sorted.len(), 2);
  assert_eq!(
    sorted
      .first()
      .and_then(|row| row.get("priority"))
      .and_then(Value::as_i64),
    Some(5)
  );

  Ok(())
}

#[tokio::test]
#[ignore = "needs a running SQL Server instance"]
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
  let completed = patched.get("completed").cloned();
  assert!(
    completed == Some(Value::Bool(true)) || completed == Some(json!(1)),
    "completed should read back as true, got {:?}",
    completed
  );

  assert!(provider.delete("test_notes", &id).await?);
  assert!(provider.find_by_id("test_notes", &id).await?.is_none());
  Ok(())
}

#[tokio::test]
#[ignore = "needs a running SQL Server instance"]
async fn transactions_commit_and_rollback_on_the_same_session() -> OrmResult<()> {
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
#[ignore = "needs a running SQL Server instance"]
async fn schema_introspection_reads_information_schema() -> OrmResult<()> {
  let provider = fresh_provider().await?;

  let collections = provider.list_collections().await?;
  assert!(collections.iter().any(|meta| meta.name == "test_notes"));

  let schema = provider.describe_collection("test_notes").await?;
  assert!(schema.fields.contains_key("title"));
  assert!(schema
    .fields
    .get("title")
    .map(|field| field.nullable)
    .unwrap_or(false));

  let database = provider.get_database_name().await?;
  assert!(!database.is_empty());
  Ok(())
}

#[tokio::test]
#[ignore = "needs a running SQL Server instance"]
async fn tableless_drop_index_is_refused_with_a_reason() -> OrmResult<()> {
  let provider = fresh_provider().await?;
  let result = Storage::drop_index(&provider, "test_notes", "missing_index").await;
  assert!(matches!(result, Err(OrmError::NotSupported(_))));
  Ok(())
}
