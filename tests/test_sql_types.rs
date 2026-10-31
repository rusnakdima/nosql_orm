//! The SQL type definitions and the dialect's own conventions.
//!
//! The legacy `*_sql` statement builders were removed — they had no caller
//! outside the test that exercised them, and they duplicated the `build_*`
//! family that the providers actually use. The types they rendered are
//! still public API, and they are covered here.

use nosql_orm::query::Filter;
use nosql_orm::sql::types::SqlForeignKey;
use nosql_orm::sql::{
  SqlColumnDef, SqlColumnType, SqlDialect, SqlIndexDef, SqlIndexType, SqlQueryBuilder, SqlTableDef,
};

#[test]
fn the_builder_carries_its_dialect() {
  assert_eq!(
    SqlQueryBuilder::new(SqlDialect::SQLite).dialect(),
    SqlDialect::SQLite
  );
  assert_eq!(
    SqlQueryBuilder::new(SqlDialect::PostgreSQL).dialect(),
    SqlDialect::PostgreSQL
  );
  assert_eq!(
    SqlQueryBuilder::new(SqlDialect::MySQL).dialect(),
    SqlDialect::MySQL
  );
}

#[test]
fn each_dialect_quotes_with_its_own_convention() {
  assert_eq!(SqlDialect::SQLite.quote_identifier("users"), "\"users\"");
  assert_eq!(
    SqlDialect::PostgreSQL.quote_identifier("users"),
    "\"users\""
  );
  assert_eq!(SqlDialect::MySQL.quote_identifier("users"), "`users`");
}

#[test]
fn each_dialect_has_its_own_parameter_placeholder() {
  assert_eq!(SqlDialect::SQLite.parameter_placeholder(0), "?");
  assert_eq!(SqlDialect::SQLite.parameter_placeholder(5), "?");
  assert_eq!(SqlDialect::PostgreSQL.parameter_placeholder(0), "$1");
  assert_eq!(SqlDialect::PostgreSQL.parameter_placeholder(5), "$6");
  assert_eq!(SqlDialect::MySQL.parameter_placeholder(0), "?");
}

#[test]
fn only_two_dialects_support_batch_and_on_conflict() {
  assert!(SqlDialect::PostgreSQL.supports_batch());
  assert!(SqlDialect::SQLite.supports_batch());
  assert!(!SqlDialect::MySQL.supports_batch());

  assert!(SqlDialect::PostgreSQL.supports_on_conflict());
  assert!(SqlDialect::SQLite.supports_on_conflict());
  assert!(!SqlDialect::MySQL.supports_on_conflict());
}

#[test]
fn column_types_render_per_dialect() {
  assert_eq!(SqlColumnType::Integer.to_sql(SqlDialect::SQLite), "INTEGER");
  assert_eq!(SqlColumnType::Text.to_sql(SqlDialect::SQLite), "TEXT");
  assert_eq!(SqlColumnType::Boolean.to_sql(SqlDialect::SQLite), "BOOLEAN");
  assert_eq!(SqlColumnType::JsonB.to_sql(SqlDialect::PostgreSQL), "JSONB");
  assert_eq!(SqlColumnType::Json.to_sql(SqlDialect::MySQL), "JSON");
  assert_eq!(
    SqlColumnType::VarChar(100).to_sql(SqlDialect::MySQL),
    "VARCHAR(100)"
  );
}

#[test]
fn index_types_render_their_name() {
  assert_eq!(SqlIndexType::BTree.to_sql(), "BTREE");
  assert_eq!(SqlIndexType::Hash.to_sql(), "HASH");
  assert_eq!(SqlIndexType::GIN.to_sql(), "GIN");
}

#[test]
fn a_table_definition_carries_its_columns_and_primary_key() {
  let table = SqlTableDef::new("users")
    .primary_key(vec!["id".to_string()])
    .if_not_exists()
    .add_column(SqlColumnDef::new("id", SqlColumnType::Integer).primary_key())
    .add_column(SqlColumnDef::new("name", SqlColumnType::Text))
    .add_column(SqlColumnDef::new("email", SqlColumnType::Text).nullable());

  assert_eq!(table.name, "users");
  assert!(table.if_not_exists);
  assert_eq!(table.columns.len(), 3);

  let sql = table.to_sql(SqlDialect::SQLite);
  assert!(sql.contains("CREATE TABLE"));
  assert!(sql.contains("\"users\""));
  assert!(sql.contains("\"id\""));
  assert!(sql.contains("\"name\""));
  assert!(sql.contains("\"email\""));
  assert!(sql.contains("PRIMARY KEY"));
}

#[test]
fn a_column_definition_carries_its_constraints() {
  let col = SqlColumnDef::new("email", SqlColumnType::VarChar(255))
    .nullable()
    .unique()
    .default("''");

  assert_eq!(col.name, "email");
  assert!(col.nullable);
  assert!(col.unique);
  assert_eq!(col.default, Some("''".to_string()));

  let sql = col.to_sql(SqlDialect::SQLite);
  assert!(sql.contains("\"email\""));
  assert!(sql.contains("VARCHAR(255)"));
}

#[test]
fn a_column_definition_renders_its_reference() {
  let col = SqlColumnDef::new("user_id", SqlColumnType::Integer).references("users", "id");

  let sql = col.to_sql(SqlDialect::SQLite);
  assert!(sql.contains("REFERENCES \"users\"(\"id\")"));
}

#[test]
fn a_foreign_key_renders_its_reference() {
  let fk = SqlForeignKey::new(vec!["user_id".to_string()], "users", vec!["id".to_string()]);

  let sql = fk.to_sql(SqlDialect::SQLite);
  assert!(sql.contains("FOREIGN KEY"));
  assert!(sql.contains("user_id"));
  assert!(sql.contains("users"));
  assert!(sql.contains("id"));
}

#[test]
fn an_index_definition_carries_its_shape() {
  let index = SqlIndexDef::new("idx_name", "users", vec!["name".to_string()])
    .unique()
    .index_type(SqlIndexType::Hash)
    .concurrently();

  assert!(index.unique);
  assert_eq!(index.index_type, SqlIndexType::Hash);
  assert!(index.concurrently);
}

#[test]
fn the_index_builder_renders_the_create_statement() {
  let builder = SqlQueryBuilder::new(SqlDialect::SQLite);

  let index = SqlIndexDef::new("idx_email", "users", vec!["email".to_string()]);
  let sql = builder.build_create_index(&index);
  assert!(sql.contains("CREATE INDEX"));
  assert!(sql.contains("\"idx_email\""));
  assert!(sql.contains("\"email\""));

  let unique = SqlIndexDef::new("idx_email_unique", "users", vec!["email".to_string()]).unique();
  assert!(builder
    .build_create_index(&unique)
    .contains("CREATE UNIQUE INDEX"));
}

#[test]
fn the_insert_builder_names_its_columns_and_placeholders() {
  let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);

  let sql = builder.build_insert("users", &["name", "email"], 2);
  assert_eq!(
    sql,
    "INSERT INTO \"users\" (\"name\", \"email\") VALUES ($1, $2)"
  );

  let sqlite = SqlQueryBuilder::new(SqlDialect::SQLite).build_insert("users", &["name"], 1);
  assert_eq!(sqlite, "INSERT INTO \"users\" (\"name\") VALUES (?)");
}

#[test]
fn a_like_filter_renders_its_pattern() {
  let sql = SqlQueryBuilder::new(SqlDialect::SQLite)
    .filter_to_sql(&Filter::Like("name".to_string(), "%test%".to_string()))
    .expect("filter_to_sql failed");
  assert!(sql.contains("LIKE"));
  assert!(sql.contains("%test%"));
}

#[test]
fn an_is_not_null_filter_names_its_field() {
  let sql = SqlQueryBuilder::new(SqlDialect::SQLite)
    .filter_to_sql(&Filter::IsNotNull("email".to_string()))
    .expect("filter_to_sql failed");
  assert_eq!(sql, "\"email\" IS NOT NULL");
}

#[test]
fn a_between_filter_renders_both_bounds() {
  let sql = SqlQueryBuilder::new(SqlDialect::SQLite)
    .filter_to_sql(&Filter::Between(
      "age".to_string(),
      serde_json::json!(18),
      serde_json::json!(65),
    ))
    .expect("filter_to_sql failed");
  assert!(sql.contains("BETWEEN"));
  assert!(sql.contains("18"));
  assert!(sql.contains("65"));
}

#[test]
fn an_ends_with_filter_renders_its_pattern() {
  let sql = SqlQueryBuilder::new(SqlDialect::SQLite)
    .filter_to_sql(&Filter::EndsWith(
      "email".to_string(),
      "@example.com".to_string(),
    ))
    .expect("filter_to_sql failed");
  assert!(sql.contains("LIKE"));
  assert!(sql.contains("%@example.com"));
}

#[test]
fn the_query_builder_produces_a_filter_the_sql_builder_renders() {
  use nosql_orm::query::QueryBuilder;

  let filter = QueryBuilder::new()
    .where_like("name", "%admin%")
    .where_gte("age", serde_json::json!(18))
    .limit(10)
    .build_filter()
    .expect("build_filter failed");

  let sql = SqlQueryBuilder::new(SqlDialect::SQLite)
    .filter_to_sql(&filter)
    .expect("filter_to_sql failed");
  assert!(sql.contains("LIKE"));
  assert!(sql.contains("%admin%"));
  assert!(sql.contains(">="));

  let between = QueryBuilder::new()
    .where_between("age", serde_json::json!(18), serde_json::json!(65))
    .build_filter()
    .expect("build_filter failed");
  let sql = SqlQueryBuilder::new(SqlDialect::SQLite)
    .filter_to_sql(&between)
    .expect("filter_to_sql failed");
  assert!(sql.contains("BETWEEN"));

  let not_in = QueryBuilder::new()
    .where_not_in(
      "status",
      vec![serde_json::json!("deleted"), serde_json::json!("archived")],
    )
    .build_filter()
    .expect("build_filter failed");
  let sql = SqlQueryBuilder::new(SqlDialect::SQLite)
    .filter_to_sql(&not_in)
    .expect("filter_to_sql failed");
  assert!(sql.contains("NOT IN"));
}

#[test]
fn a_filter_matches_a_document() {
  let like = Filter::Like("name".to_string(), "%test".to_string());
  assert!(like.matches(&serde_json::json!({ "name": "my test value" })));

  let between = Filter::Between(
    "age".to_string(),
    serde_json::json!(18),
    serde_json::json!(65),
  );
  assert!(between.matches(&serde_json::json!({ "age": 25 })));
  assert!(!between.matches(&serde_json::json!({ "age": 17 })));
  assert!(!between.matches(&serde_json::json!({ "age": 70 })));

  let not_null = Filter::IsNotNull("email".to_string());
  assert!(not_null.matches(&serde_json::json!({ "email": "test@example.com" })));
  assert!(!not_null.matches(&serde_json::json!({ "email": null })));
  assert!(!not_null.matches(&serde_json::json!({})));
}
