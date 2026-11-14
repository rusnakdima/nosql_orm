// Translation-unit tests for the ClickHouse statement rewrites.
//
// No server is needed: these cover the parts of the provider that turn the
// SQL the shared builders emit into SQL ClickHouse accepts, plus the parsing
// of the `JSON` response document.

use super::*;

#[test]
fn literal_question_marks_are_doubled_and_bind_slots_are_not() {
  let sql = "INSERT INTO notes (id, title) VALUES ('why?', 'x') WHERE id = ?";
  assert_eq!(
    escape_literal_question_marks(sql),
    "INSERT INTO notes (id, title) VALUES ('why??', 'x') WHERE id = ?"
  );
}

#[test]
fn a_doubled_quote_inside_a_literal_keeps_the_literal_open() {
  let sql = "SELECT * FROM t WHERE note = 'it''s ?' AND id = ?";
  assert_eq!(
    escape_literal_question_marks(sql),
    "SELECT * FROM t WHERE note = 'it''s ??' AND id = ?"
  );
}

#[test]
fn update_becomes_a_mutation() {
  let rewritten = rewrite_update("UPDATE notes SET title = 'x' WHERE id = ?").expect("rewrite");
  assert_eq!(
    rewritten,
    "ALTER TABLE notes UPDATE title = 'x' WHERE id = ?"
  );
}

#[test]
fn plain_reads_are_passed_through() {
  assert!(rewrite_update("SELECT * FROM notes").is_none());
}

#[test]
fn create_index_becomes_a_skip_index() {
  let rewritten = rewrite_create_index("CREATE INDEX idx_title ON notes (title)")
    .expect("result")
    .expect("rewrite");
  assert_eq!(
    rewritten,
    "ALTER TABLE notes ADD INDEX idx_title (title) TYPE minmax GRANULARITY 1"
  );
}

#[test]
fn unique_index_creation_is_refused_rather_than_downgraded() {
  let result = rewrite_create_index("CREATE UNIQUE INDEX idx_title ON notes (title)");
  assert!(matches!(result, Err(OrmError::NotSupported(_))));
}

#[test]
fn drop_index_uses_the_mysql_shape_clickhouse_understands() {
  assert_eq!(
    rewrite_drop_index("DROP INDEX idx_title ON notes"),
    Some("ALTER TABLE notes DROP INDEX idx_title".to_string())
  );
  assert_eq!(rewrite_drop_index("SELECT 1"), None);
}

#[test]
fn create_table_gains_an_engine_and_loses_text() {
  let rewritten =
    rewrite_create_table("CREATE TABLE notes (id TEXT PRIMARY KEY)").expect("rewrite");
  assert_eq!(
    rewritten,
    "CREATE TABLE notes (id String) ENGINE = MergeTree ORDER BY tuple()"
  );
}

#[test]
fn create_table_leaves_clickhouse_native_types_alone() {
  let rewritten =
    rewrite_create_table("CREATE TABLE notes (name String) ENGINE = MergeTree").expect("rewrite");
  assert_eq!(
    rewritten,
    "CREATE TABLE notes (name String) ENGINE = MergeTree ORDER BY tuple()"
  );
}

#[test]
fn mutation_target_splits_the_table_from_the_condition() {
  assert_eq!(
    mutation_target("DELETE FROM notes WHERE id = ?"),
    Some(("notes".to_string(), Some("id = ?".to_string())))
  );
  assert_eq!(
    mutation_target("UPDATE notes SET a = 1 WHERE b = ?"),
    Some(("notes".to_string(), Some("b = ?".to_string())))
  );
  assert_eq!(
    mutation_target("DELETE FROM notes"),
    Some(("notes".to_string(), None))
  );
  assert_eq!(mutation_target("INSERT INTO notes VALUES (1)"), None);
}

#[test]
fn credentials_are_split_out_of_the_url() {
  let (url, user, password) = split_userinfo("http://default:secret@localhost:8123");
  assert_eq!(url, "http://localhost:8123");
  assert_eq!(user.as_deref(), Some("default"));
  assert_eq!(password.as_deref(), Some("secret"));

  let (url, user, password) = split_userinfo("http://localhost:8123/default");
  assert_eq!(url, "http://localhost:8123/default");
  assert!(user.is_none());
  assert!(password.is_none());
}

#[test]
fn json_document_rows_follow_the_meta_order() {
  let body = br#"{"meta":[{"name":"id","type":"String"},{"name":"n","type":"UInt64"}],
                  "data":[["a",5]],"rows":1}"#;
  let (names, rows) = parse_json_document(body).expect("parse");
  assert_eq!(names, vec!["id".to_string(), "n".to_string()]);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].get("id"), Some(&Value::String("a".to_string())));
  assert_eq!(rows[0].get("n"), Some(&Value::Number(5.into())));
}

#[test]
fn json_document_accepts_row_objects_too() {
  let body = br#"{"meta":[{"name":"id","type":"String"}],"data":[{"id":"b"}],"rows":1}"#;
  let (names, rows) = parse_json_document(body).expect("parse");
  assert_eq!(names, vec!["id".to_string()]);
  assert_eq!(rows[0].get("id"), Some(&Value::String("b".to_string())));
}

#[test]
fn an_empty_response_is_reported_instead_of_guessed() {
  assert!(parse_json_document(b"").is_err());
}
