// Translation-unit tests for the Oracle statement rewrite.
//
// No database and no Oracle Client: these cover the translation the provider
// performs before a statement reaches the driver, and the JSON conversion of
// a bind parameter.

use super::*;

fn translate(sql: &str) -> String {
  rewrite_portable_sql(&strip_drop_if_exists(sql), PlaceholderStyle::Oracle)
}

#[test]
fn bind_slots_become_positional_oracle_parameters() {
  assert_eq!(
    translate("SELECT * FROM notes WHERE a = ? AND b = ?"),
    "SELECT * FROM notes WHERE a = :1 AND b = :2"
  );
}

#[test]
fn a_question_mark_inside_a_literal_is_not_a_parameter() {
  assert_eq!(
    translate("SELECT * FROM notes WHERE title = 'why?' AND id = ?"),
    "SELECT * FROM notes WHERE title = 'why?' AND id = :1"
  );
}

#[test]
fn boolean_literals_oracle_lacks_become_numbers() {
  assert_eq!(
    translate("UPDATE notes SET done = TRUE WHERE id = ?"),
    "UPDATE notes SET done = 1 WHERE id = :1"
  );
}

#[test]
fn limit_and_offset_become_offset_fetch_first() {
  assert_eq!(
    translate("SELECT * FROM notes ORDER BY id LIMIT 5 OFFSET 10"),
    "SELECT * FROM notes ORDER BY id OFFSET 10 ROWS FETCH NEXT 5 ROWS ONLY"
  );
  assert_eq!(
    translate("SELECT * FROM notes LIMIT 4"),
    "SELECT * FROM notes ORDER BY (SELECT NULL) OFFSET 0 ROWS FETCH NEXT 4 ROWS ONLY"
  );
}

#[test]
fn drop_statements_lose_the_if_exists_clause_oracle_before_23c_rejects() {
  assert_eq!(
    strip_drop_if_exists("DROP TABLE IF EXISTS \"notes\""),
    "DROP TABLE \"notes\""
  );
  assert_eq!(
    strip_drop_if_exists("DROP INDEX IF EXISTS \"idx_title\""),
    "DROP INDEX \"idx_title\""
  );
  // Not a DROP: the clause belongs to whoever wrote it.
  assert_eq!(
    strip_drop_if_exists("SELECT * FROM t WHERE x = ' IF EXISTS '"),
    "SELECT * FROM t WHERE x = ' IF EXISTS '"
  );
}

#[test]
fn a_bind_parameter_becomes_the_owned_driver_type() {
  assert!(matches!(
    OwnedParam::from_json(&Value::Null),
    OwnedParam::Null(_)
  ));
  assert!(matches!(
    OwnedParam::from_json(&Value::Bool(true)),
    OwnedParam::Int(1)
  ));
  assert!(matches!(
    OwnedParam::from_json(&Value::Bool(false)),
    OwnedParam::Int(0)
  ));
  assert!(matches!(
    OwnedParam::from_json(&serde_json::json!(7)),
    OwnedParam::Int(7)
  ));
  assert!(matches!(
    OwnedParam::from_json(&serde_json::json!(7.5)),
    OwnedParam::Float(v) if (v - 7.5).abs() < f64::EPSILON
  ));
  assert!(matches!(
    OwnedParam::from_json(&Value::String("x".to_string())),
    OwnedParam::Text(v) if v == "x"
  ));
}
