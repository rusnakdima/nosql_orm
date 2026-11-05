use super::utils::{statement_kind, StatementKind};

#[test]
fn select_is_a_read() {
  assert_eq!(statement_kind("SELECT * FROM users"), StatementKind::Read);
}

#[test]
fn lower_case_and_leading_space_are_reads() {
  assert_eq!(
    statement_kind("  \n select id from users"),
    StatementKind::Read
  );
}

#[test]
fn leading_line_comment_does_not_hide_the_keyword() {
  assert_eq!(
    statement_kind("-- a comment\nSELECT 1"),
    StatementKind::Read
  );
}

#[test]
fn leading_block_comment_does_not_hide_the_keyword() {
  assert_eq!(
    statement_kind("/* a comment */ SHOW TABLES"),
    StatementKind::Read
  );
}

#[test]
fn other_row_returning_statements_are_reads() {
  for sql in [
    "WITH recent AS (SELECT id FROM users) SELECT * FROM recent",
    "EXPLAIN SELECT 1",
    "VALUES (1), (2)",
    "TABLE users",
    "PRAGMA table_info(users)",
    "DESCRIBE users",
  ] {
    assert_eq!(statement_kind(sql), StatementKind::Read, "{sql}");
  }
}

#[test]
fn plain_writes_are_writes() {
  for sql in [
    "INSERT INTO users (id) VALUES ('a')",
    "UPDATE users SET name = 'b'",
    "DELETE FROM users",
    "CREATE TABLE users (id TEXT)",
    "ALTER TABLE users ADD COLUMN name TEXT",
    "DROP TABLE users",
  ] {
    assert_eq!(statement_kind(sql), StatementKind::Write, "{sql}");
  }
}

#[test]
fn a_write_with_returning_returns_rows() {
  assert_eq!(
    statement_kind("INSERT INTO users (id) VALUES ('a') RETURNING id"),
    StatementKind::Read
  );
  assert_eq!(
    statement_kind("update users set name = 'b' returning *"),
    StatementKind::Read
  );
}
