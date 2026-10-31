//! SQL projections must name the columns the database actually has.
//!
//! An exclude projection names the fields to drop, so the remaining columns
//! are the table's own columns minus those — which only the database knows.
//! These tests run against a real SQLite database so the rendered statement
//! is not just plausible but executable.

use nosql_orm::error::OrmResult;
use nosql_orm::prelude::AdminCommands;
use nosql_orm::providers::sql::SqliteProvider;
use nosql_orm::query::Projection;
use nosql_orm::sql::introspection::TableIntrospector;

/// A provider holding a `users` table with four columns.
async fn provider_with_users_table() -> OrmResult<SqliteProvider> {
  let provider = SqliteProvider::connect(":memory:").await?;
  provider
    .execute_raw(
      "CREATE TABLE users (id TEXT PRIMARY KEY, name TEXT, email TEXT, password TEXT)",
      Vec::new(),
    )
    .await?;
  provider
    .execute_raw(
      "INSERT INTO users (id, name, email, password) VALUES ('u1', 'Ada', 'ada@example.com', 'secret')",
      Vec::new(),
    )
    .await?;
  Ok(provider)
}

#[tokio::test]
async fn exclude_projection_renders_the_remaining_columns() -> OrmResult<()> {
  let provider = provider_with_users_table().await?;

  let projection = Projection::exclude(&["password"]);
  let sql = provider
    .build_select("users", None, Some(&projection), None, None, None)
    .await?;

  assert_eq!(
    sql,
    r#"SELECT "id", "name", "email" FROM "users""#,
    "the excluded column must be dropped and the rest named"
  );
  Ok(())
}

#[tokio::test]
async fn the_rendered_statement_runs_and_returns_the_kept_columns() -> OrmResult<()> {
  let provider = provider_with_users_table().await?;

  let projection = Projection::exclude(&["password"]);
  let sql = provider
    .build_select("users", None, Some(&projection), None, None, None)
    .await?;

  let result = provider.execute_raw(&sql, Vec::new()).await?;

  assert_eq!(result.columns, vec!["id", "name", "email"]);
  assert_eq!(result.rows.len(), 1);
  assert_eq!(result.rows[0][0], serde_json::json!("u1"));
  Ok(())
}

#[tokio::test]
async fn table_columns_reports_what_the_database_has() -> OrmResult<()> {
  let provider = provider_with_users_table().await?;

  let columns = provider.table_columns("users").await?;

  assert_eq!(columns, vec!["id", "name", "email", "password"]);
  Ok(())
}
