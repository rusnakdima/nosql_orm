// Translation-unit tests for the T-SQL statement rewrite.
//
// SQL Server is not needed: these cover what `execute` and `query` do to the
// SQL the shared builders emit before it reaches the wire.

use super::*;

fn translate(sql: &str) -> String {
  rewrite_portable_sql(sql, PlaceholderStyle::Tsql)
}

#[test]
fn bind_slots_become_positional_tsql_parameters() {
  assert_eq!(
    translate("SELECT * FROM notes WHERE a = ? AND b = ?"),
    "SELECT * FROM notes WHERE a = @P1 AND b = @P2"
  );
}

#[test]
fn a_question_mark_inside_a_literal_is_not_a_parameter() {
  assert_eq!(
    translate("SELECT * FROM notes WHERE title = 'why?' AND id = ?"),
    "SELECT * FROM notes WHERE title = 'why?' AND id = @P1"
  );
}

#[test]
fn a_question_mark_inside_a_quoted_identifier_is_not_a_parameter() {
  assert_eq!(
    translate("SELECT * FROM notes WHERE \"why?\" = ?"),
    "SELECT * FROM notes WHERE \"why?\" = @P1"
  );
}

#[test]
fn boolean_literals_tsql_lacks_become_numbers() {
  assert_eq!(
    translate("UPDATE notes SET done = TRUE WHERE id = ?"),
    "UPDATE notes SET done = 1 WHERE id = @P1"
  );
  assert_eq!(
    translate("UPDATE notes SET done = FALSE WHERE id = ?"),
    "UPDATE notes SET done = 0 WHERE id = @P1"
  );
}

#[test]
fn a_quoted_identifier_spelled_true_is_left_alone() {
  assert_eq!(
    translate("SELECT \"TRUE\" FROM notes WHERE id = ?"),
    "SELECT \"TRUE\" FROM notes WHERE id = @P1"
  );
}

#[test]
fn limit_and_offset_become_offset_fetch_next() {
  assert_eq!(
    translate("SELECT * FROM notes ORDER BY id LIMIT 5 OFFSET 10"),
    "SELECT * FROM notes ORDER BY id OFFSET 10 ROWS FETCH NEXT 5 ROWS ONLY"
  );
  assert_eq!(
    translate("SELECT * FROM notes ORDER BY id LIMIT 5"),
    "SELECT * FROM notes ORDER BY id OFFSET 0 ROWS FETCH NEXT 5 ROWS ONLY"
  );
}

#[test]
fn fetch_without_an_order_by_synthesizes_one() {
  assert_eq!(
    translate("SELECT * FROM notes LIMIT 5"),
    "SELECT * FROM notes ORDER BY (SELECT NULL) OFFSET 0 ROWS FETCH NEXT 5 ROWS ONLY"
  );
}

#[test]
fn offset_alone_stays_legal() {
  assert_eq!(
    translate("SELECT * FROM notes ORDER BY id OFFSET 3"),
    "SELECT * FROM notes ORDER BY id OFFSET 3 ROWS"
  );
}

#[test]
fn statements_without_limit_pass_through() {
  assert_eq!(
    translate("DELETE FROM notes WHERE id = @P1"),
    "DELETE FROM notes WHERE id = @P1"
  );
  assert_eq!(translate("BEGIN TRANSACTION"), "BEGIN TRANSACTION");
}

#[test]
fn the_shared_create_table_types_are_remapped() {
  let rewritten =
    rewrite_create_table_tsql("CREATE TABLE \"notes\" (id TEXT PRIMARY KEY)").expect("rewrite");
  assert_eq!(
    rewritten,
    "CREATE TABLE \"notes\" (id NVARCHAR(450) PRIMARY KEY)"
  );

  let rewritten =
    rewrite_create_table_tsql("CREATE TABLE t (flag BOOLEAN, n INTEGER, when_at TIMESTAMP)")
      .expect("rewrite");
  assert_eq!(
    rewritten,
    "CREATE TABLE t (flag BIT, n INT, when_at DATETIME2)"
  );
}

#[test]
fn create_table_with_a_length_keeps_the_length() {
  let rewritten =
    rewrite_create_table_tsql("CREATE TABLE t (title VARCHAR(100), body TEXT)").expect("rewrite");
  assert_eq!(
    rewritten,
    "CREATE TABLE t (title VARCHAR(100), body NVARCHAR(450))"
  );
}

#[test]
fn the_shared_drop_index_statement_is_refused_with_a_reason() {
  assert!(is_tableless_drop_index("DROP INDEX IF EXISTS \"idx\""));
  assert!(!is_tableless_drop_index("DROP INDEX \"idx\" ON \"notes\""));
  assert!(!is_tableless_drop_index("DELETE FROM notes"));
}
