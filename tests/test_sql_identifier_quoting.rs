//! A quoted identifier cannot be escaped.
//!
//! The filter compiler renders every field name through the dialect's quoting
//! convention. A name that needs quoting — one carrying a quote character, a
//! space or a dash — must come out as a single quoted identifier, with the
//! dialect's quote character doubled inside it. A name that carries SQL
//! syntax (`users"; DROP TABLE users; --`) must therefore stay one
//! identifier instead of terminating the statement.

use nosql_orm::query::Filter;
use nosql_orm::sql::{SqlDialect, SqlQueryBuilder};

/// Compile `field = 'x'` for the given dialect.
fn compiled(dialect: SqlDialect, field: &str) -> String {
  SqlQueryBuilder::new(dialect)
    .filter_to_sql(&Filter::Eq(field.to_string(), serde_json::json!("x")))
    .expect("filter_to_sql must accept a quoted identifier")
}

/// Count the identifier delimiters that are not doubled (escaped).
///
/// A correctly escaped identifier has exactly two: the one that opens it and
/// the one that closes it. An escaped-away quote would show up as more.
fn undoubled_delimiters(sql: &str, delimiter: char) -> usize {
  let mut count = 0;
  let mut chars = sql.chars().peekable();
  while let Some(c) = chars.next() {
    if c == delimiter {
      match chars.peek() {
        Some(&next) if next == delimiter => {
          chars.next();
        }
        _ => count += 1,
      }
    }
  }
  count
}

/// The text of `sql` that lies outside the first quoted identifier.
///
/// Scans the identifier the way a database does: a doubled delimiter is an
/// escaped delimiter and does not end the identifier.
fn outside_the_identifier(sql: &str, delimiter: char) -> String {
  let mut chars = sql.chars().peekable();
  let mut outside = String::new();

  // Text before the opening delimiter.
  for c in chars.by_ref() {
    if c == delimiter {
      break;
    }
    outside.push(c);
  }

  // The identifier itself.
  while let Some(c) = chars.next() {
    if c == delimiter {
      match chars.peek() {
        Some(&next) if next == delimiter => {
          chars.next();
        }
        _ => break,
      }
    }
  }

  // Text after the closing delimiter.
  for c in chars {
    outside.push(c);
  }

  outside
}

#[test]
fn a_plain_identifier_is_unchanged() {
  assert_eq!(compiled(SqlDialect::PostgreSQL, "name"), "\"name\" = 'x'");
  assert_eq!(compiled(SqlDialect::SQLite, "name"), "\"name\" = 'x'");
  assert_eq!(compiled(SqlDialect::MySQL, "name"), "`name` = 'x'");
}

#[test]
fn postgres_doubles_an_embedded_double_quote() {
  assert_eq!(
    compiled(SqlDialect::PostgreSQL, "user\"name"),
    "\"user\"\"name\" = 'x'"
  );
}

#[test]
fn sqlite_doubles_an_embedded_double_quote() {
  assert_eq!(
    compiled(SqlDialect::SQLite, "user\"name"),
    "\"user\"\"name\" = 'x'"
  );
}

#[test]
fn mysql_doubles_an_embedded_backtick() {
  assert_eq!(
    compiled(SqlDialect::MySQL, "user`name"),
    "`user``name` = 'x'"
  );
}

#[test]
fn an_identifier_with_a_space_is_quoted_not_rejected() {
  assert_eq!(
    compiled(SqlDialect::PostgreSQL, "my column"),
    "\"my column\" = 'x'"
  );
  assert_eq!(
    compiled(SqlDialect::MySQL, "my column"),
    "`my column` = 'x'"
  );
}

#[test]
fn a_drop_table_payload_stays_inside_one_identifier() {
  let sql = compiled(SqlDialect::PostgreSQL, "users\"; DROP TABLE users; --");

  assert_eq!(sql, "\"users\"\"; DROP TABLE users; --\" = 'x'");
  // Exactly one identifier: the payload never closes it.
  assert_eq!(undoubled_delimiters(&sql, '"'), 2);
}

#[test]
fn a_drop_table_payload_stays_inside_one_mysql_identifier() {
  let sql = compiled(SqlDialect::MySQL, "users`; DROP TABLE users; --");

  assert_eq!(sql, "`users``; DROP TABLE users; --` = 'x'");
  assert_eq!(undoubled_delimiters(&sql, '`'), 2);
}

#[test]
fn a_payload_cannot_open_a_second_statement() {
  let sql = compiled(SqlDialect::PostgreSQL, "a\"; SELECT 1; --");

  // The only semicolons are inside the quoted identifier.
  assert_eq!(sql, "\"a\"\"; SELECT 1; --\" = 'x'");
  assert_eq!(undoubled_delimiters(&sql, '"'), 2);
  assert_eq!(outside_the_identifier(&sql, '"'), " = 'x'");
}

#[test]
fn a_reserved_word_is_quoted_not_rejected() {
  // A quoted keyword is an ordinary identifier; rejecting it would refuse to
  // address a column that is legitimately called `order`.
  assert_eq!(compiled(SqlDialect::PostgreSQL, "order"), "\"order\" = 'x'");
  assert_eq!(compiled(SqlDialect::MySQL, "order"), "`order` = 'x'");
}
