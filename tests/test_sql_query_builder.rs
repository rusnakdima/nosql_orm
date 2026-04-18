use nosql_orm::sql::{SqlColumnDef, SqlColumnType, SqlDialect, SqlIndexDef, SqlIndexType, SqlTableDef};
use nosql_orm::sql::types::SqlForeignKey;
use nosql_orm::sql::SqlQueryBuilder;

#[test]
fn test_sqlite_dialect() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
    assert_eq!(builder.dialect(), SqlDialect::SQLite);
}

#[test]
fn test_postgres_dialect() {
    let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);
    assert_eq!(builder.dialect(), SqlDialect::PostgreSQL);
}

#[test]
fn test_mysql_dialect() {
    let builder = SqlQueryBuilder::new(SqlDialect::MySQL);
    assert_eq!(builder.dialect(), SqlDialect::MySQL);
}

#[test]
fn test_quote_identifier_sqlite() {
    let dialect = SqlDialect::SQLite;
    assert_eq!(dialect.quote_identifier("users"), "\"users\"".to_string());
}

#[test]
fn test_quote_identifier_postgres() {
    let dialect = SqlDialect::PostgreSQL;
    assert_eq!(dialect.quote_identifier("users"), "\"users\"".to_string());
}

#[test]
fn test_quote_identifier_mysql() {
    let dialect = SqlDialect::MySQL;
    assert_eq!(dialect.quote_identifier("users"), "`users`".to_string());
}

#[test]
fn test_create_table_sqlite() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);

    let columns = vec![
        SqlColumnDef::new("id", SqlColumnType::Integer).primary_key(),
        SqlColumnDef::new("name", SqlColumnType::Text),
        SqlColumnDef::new("email", SqlColumnType::Text).nullable(),
    ];

    let table = SqlTableDef::new("users")
        .add_column(SqlColumnDef::new("id", SqlColumnType::Integer).primary_key())
        .add_column(SqlColumnDef::new("name", SqlColumnType::Text))
        .add_column(SqlColumnDef::new("email", SqlColumnType::Text).nullable());

    let sql = builder.create_table_sql(&table);
    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("\"users\""));
    assert!(sql.contains("\"id\""));
    assert!(sql.contains("\"name\""));
    assert!(sql.contains("\"email\""));
}

#[test]
fn test_create_table_with_primary_key() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);

    let table = SqlTableDef::new("test")
        .primary_key(vec!["id".to_string()])
        .add_column(SqlColumnDef::new("id", SqlColumnType::Integer));

    let sql = builder.create_table_sql(&table);
    assert!(sql.contains("PRIMARY KEY"));
}

#[test]
fn test_create_index_sql() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);

    let index = SqlIndexDef::new("idx_email", "users", vec!["email".to_string()]);

    let sql = builder.create_index_sql("users", &index);
    assert!(sql.contains("CREATE INDEX"));
    assert!(sql.contains("idx_email"));
    assert!(sql.contains("\"email\""));
}

#[test]
fn test_create_unique_index_sql() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);

    let index = SqlIndexDef::new("idx_email_unique", "users", vec!["email".to_string()]).unique();

    let sql = builder.create_index_sql("users", &index);
    assert!(sql.contains("CREATE UNIQUE INDEX"));
}

#[test]
fn test_drop_table_sql() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
    let sql = builder.drop_table_sql("users");
    assert!(sql.contains("DROP TABLE"));
    assert!(sql.contains("\"users\""));
}

#[test]
fn test_insert_sql() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
    let sql = builder.insert_sql("users", &serde_json::json!({
        "name": "Alice",
        "email": "alice@example.com"
    }));

    assert!(sql.contains("INSERT INTO"));
    assert!(sql.contains("\"users\""));
    assert!(sql.contains("\"name\""));
    assert!(sql.contains("\"email\""));
    assert!(sql.contains("?"));
}

#[test]
fn test_update_sql() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
    let sql = builder.update_sql(
        "users",
        &serde_json::json!({"name": "Bob"}),
        "id",
        "1",
    );

    assert!(sql.contains("UPDATE"));
    assert!(sql.contains("\"users\""));
    assert!(sql.contains("SET"));
    assert!(sql.contains("\"name\""));
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("\"id\""));
    assert!(sql.contains("?"));
}

#[test]
fn test_delete_sql() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
    let sql = builder.delete_sql("users", "id", "1");

    assert!(sql.contains("DELETE FROM"));
    assert!(sql.contains("\"users\""));
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("\"id\""));
    assert!(sql.contains("?"));
}

#[test]
fn test_select_sql_basic() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
    let sql = builder.select_sql("users", None, None, None);

    assert!(sql.contains("SELECT * FROM"));
    assert!(sql.contains("\"users\""));
}

#[test]
fn test_select_sql_with_limit() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
    let sql = builder.select_sql("users", None, Some(10), None);

    assert!(sql.contains("LIMIT 10"));
}

#[test]
fn test_select_sql_with_offset() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
    let sql = builder.select_sql("users", None, Some(10), Some(20));

    assert!(sql.contains("LIMIT 10"));
    assert!(sql.contains("OFFSET 20"));
}

#[test]
fn test_select_sql_with_order() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);
    let sql = builder.select_sql("users", None, None, None);

    assert!(sql.contains("ORDER BY"));
}

#[test]
fn test_postgres_jsonb_type() {
    let builder = SqlQueryBuilder::new(SqlDialect::PostgreSQL);

    let table = SqlTableDef::new("documents")
        .add_column(SqlColumnDef::new("id", SqlColumnType::Serial).primary_key())
        .add_column(SqlColumnDef::new("data", SqlColumnType::JsonB).nullable());

    let sql = builder.create_table_sql(&table);
    assert!(sql.contains("JSONB"));
}

#[test]
fn test_mysql_varchar_type() {
    let builder = SqlQueryBuilder::new(SqlDialect::MySQL);

    let table = SqlTableDef::new("users")
        .add_column(SqlColumnDef::new("id", SqlColumnType::Integer).primary_key())
        .add_column(SqlColumnDef::new("name", SqlColumnType::VarChar(255)));

    let sql = builder.create_table_sql(&table);
    assert!(sql.contains("VARCHAR(255)"));
    assert!(sql.contains("`users`"));
}

#[test]
fn test_sql_column_type_to_sql() {
    assert_eq!(SqlColumnType::Integer.to_sql(SqlDialect::SQLite), "INTEGER");
    assert_eq!(SqlColumnType::Text.to_sql(SqlDialect::SQLite), "TEXT");
    assert_eq!(SqlColumnType::Boolean.to_sql(SqlDialect::SQLite), "BOOLEAN");
    assert_eq!(SqlColumnType::JsonB.to_sql(SqlDialect::PostgreSQL), "JSONB");
    assert_eq!(SqlColumnType::Json.to_sql(SqlDialect::MySQL), "JSON");
    assert_eq!(SqlColumnType::VarChar(100).to_sql(SqlDialect::MySQL), "VARCHAR(100)");
}

#[test]
fn test_sql_index_type_to_sql() {
    assert_eq!(SqlIndexType::BTree.to_sql(), "BTREE");
    assert_eq!(SqlIndexType::Hash.to_sql(), "HASH");
    assert_eq!(SqlIndexType::GIN.to_sql(), "GIN");
}

#[test]
fn test_sqlite_parameter_placeholder() {
    let dialect = SqlDialect::SQLite;
    assert_eq!(dialect.parameter_placeholder(0), "?");
    assert_eq!(dialect.parameter_placeholder(5), "?");
}

#[test]
fn test_postgres_parameter_placeholder() {
    let dialect = SqlDialect::PostgreSQL;
    assert_eq!(dialect.parameter_placeholder(0), "$1");
    assert_eq!(dialect.parameter_placeholder(5), "$6");
}

#[test]
fn test_table_def_builder() {
    let table = SqlTableDef::new("users")
        .primary_key(vec!["id".to_string()])
        .if_not_exists();

    assert_eq!(table.name, "users");
    assert!(table.if_not_exists);
}

#[test]
fn test_column_def_builder() {
    let col = SqlColumnDef::new("email", SqlColumnType::VarChar(255))
        .nullable()
        .unique()
        .default("''");

    assert_eq!(col.name, "email");
    assert!(col.nullable);
    assert!(col.unique);
    assert_eq!(col.default, Some("''".to_string()));
}

#[test]
fn test_foreign_key_builder() {
    let fk = SqlForeignKey::new(
        vec!["user_id".to_string()],
        "users",
        vec!["id".to_string()],
    );

    let sql = fk.to_sql(SqlDialect::SQLite);
    assert!(sql.contains("FOREIGN KEY"));
    assert!(sql.contains("user_id"));
    assert!(sql.contains("users"));
    assert!(sql.contains("id"));
}

#[test]
fn test_index_def_builder() {
    let index = SqlIndexDef::new("idx_name", "users", vec!["name".to_string()])
        .unique()
        .index_type(SqlIndexType::Hash)
        .concurrently();

    assert!(index.unique);
    assert_eq!(index.index_type, SqlIndexType::Hash);
    assert!(index.concurrently);
}

#[test]
fn test_create_table_with_foreign_key() {
    let builder = SqlQueryBuilder::new(SqlDialect::SQLite);

    let table = SqlTableDef::new("orders")
        .add_column(SqlColumnDef::new("id", SqlColumnType::Integer).primary_key())
        .add_column(SqlColumnDef::new("user_id", SqlColumnType::Integer))
        .add_column(SqlColumnDef::new("amount", SqlColumnType::Float));

    let sql = builder.create_table_sql(&table);
    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("\"orders\""));
}

#[test]
fn test_all_sql_dialects_quote() {
    let name = "test_table";
    assert_eq!(SqlDialect::SQLite.quote_identifier(name), "\"test_table\"");
    assert_eq!(SqlDialect::PostgreSQL.quote_identifier(name), "\"test_table\"");
    assert_eq!(SqlDialect::MySQL.quote_identifier(name), "`test_table`");
}

#[test]
fn test_dialect_supports_batch() {
    assert!(SqlDialect::PostgreSQL.supports_batch());
    assert!(SqlDialect::SQLite.supports_batch());
    assert!(!SqlDialect::MySQL.supports_batch());
}

#[test]
fn test_dialect_supports_on_conflict() {
    assert!(SqlDialect::PostgreSQL.supports_on_conflict());
    assert!(SqlDialect::SQLite.supports_on_conflict());
    assert!(!SqlDialect::MySQL.supports_on_conflict());
}