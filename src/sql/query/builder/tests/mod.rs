use super::*;
use crate::query::Projection;
use crate::sql::types::SqlDialect;

#[test]
fn test_simple_filter() {
  let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
  let filter = crate::query::Filter::Eq("name".to_string(), serde_json::json!("Alice"));
  let sql = builder
    .filter_to_sql(&filter)
    .expect("filter_to_sql failed");
  assert_eq!(sql, "\"name\" = 'Alice'");
}

#[test]
fn test_compound_filter() {
  let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
  let filter = crate::query::Filter::And(vec![
    crate::query::Filter::Eq("age".to_string(), serde_json::json!(25)),
    crate::query::Filter::Gt("balance".to_string(), serde_json::json!(100)),
  ]);
  let sql = builder
    .filter_to_sql(&filter)
    .expect("filter_to_sql failed");
  assert!(sql.contains("\"age\" = 25"));
  assert!(sql.contains("\"balance\" > 100"));
}

#[test]
fn test_insert_sql() {
  let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
  let sql = builder.build_insert("users", &["name", "email"], 2);
  assert_eq!(
    sql,
    "INSERT INTO \"users\" (\"name\", \"email\") VALUES ($1, $2)"
  );
}

#[test]
fn test_select_with_projection() {
  let builder = SqlQueryBuilder::new(SqlDialect::MySQL);
  let projection = Projection {
    select: Some(vec!["id".to_string(), "name".to_string()]),
    exclude: None,
  };
  let sql = builder
    .build_select("users", None, Some(&projection), None, None, None, None)
    .expect("build_select failed");
  assert_eq!(sql, "SELECT `id`, `name` FROM `users`");
}

#[test]
fn test_mysql_quote() {
  let builder = SqlQueryBuilder::new(SqlDialect::MySQL);
  assert_eq!(builder.dialect().quote_identifier("name"), "`name`");
}

#[test]
fn test_filter_params_number_placeholders_in_order() {
  let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
  let filter = crate::query::Filter::And(vec![
    crate::query::Filter::Eq("name".to_string(), serde_json::json!("Alice")),
    crate::query::Filter::Between(
      "age".to_string(),
      serde_json::json!(18),
      serde_json::json!(65),
    ),
  ]);
  let (sql, params) = builder
    .filter_to_sql_params(&filter)
    .expect("filter_to_sql_params failed");
  assert_eq!(
    sql,
    "(\"name\" = $1) AND (\"age\" BETWEEN $2 AND $3)"
  );
  assert_eq!(
    params,
    vec![
      serde_json::json!("Alice"),
      serde_json::json!(18),
      serde_json::json!(65)
    ]
  );
}

#[test]
fn test_filter_params_keep_injection_as_a_value() {
  let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
  let injected = "x' OR '1'='1";
  let filter = crate::query::Filter::Eq(
    "name".to_string(),
    serde_json::json!(injected),
  );
  let (sql, params) = builder
    .filter_to_sql_params(&filter)
    .expect("filter_to_sql_params failed");
  assert_eq!(sql, "\"name\" = ?");
  assert_eq!(params, vec![serde_json::json!(injected)]);
}

#[test]
fn test_mysql_escapes_backslashes_and_control_characters() {
  let builder = SqlQueryBuilder::new(SqlDialect::MySQL);
  let literal = builder.value_to_sql(&serde_json::json!("a\\b\n'c"));
  assert_eq!(literal, "'a\\\\b\\n''c'");
}

#[test]
fn test_postgres_keeps_backslashes_literal() {
  let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
  let literal = builder.value_to_sql(&serde_json::json!("a\\b"));
  assert_eq!(literal, "'a\\b'");
}
