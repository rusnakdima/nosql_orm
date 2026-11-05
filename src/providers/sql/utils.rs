use base64::{engine::general_purpose, Engine as _};

pub fn base64_encode(data: &[u8]) -> String {
  general_purpose::STANDARD.encode(data)
}

#[cfg_attr(not(feature = "sql-postgres"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatementKind {
  Read,
  Write,
}

#[cfg_attr(not(feature = "sql-postgres"), allow(dead_code))]
pub(crate) fn statement_kind(sql: &str) -> StatementKind {
  let keyword = first_keyword(sql);
  if [
    "SELECT", "WITH", "SHOW", "DESCRIBE", "DESC", "EXPLAIN", "VALUES", "TABLE", "PRAGMA",
  ]
  .iter()
  .any(|read| keyword.eq_ignore_ascii_case(read))
  {
    return StatementKind::Read;
  }

  if ["INSERT", "UPDATE", "DELETE", "REPLACE", "MERGE"]
    .iter()
    .any(|write| keyword.eq_ignore_ascii_case(write))
    && contains_keyword(sql, "RETURNING")
  {
    return StatementKind::Read;
  }

  StatementKind::Write
}

fn first_keyword(sql: &str) -> &str {
  let mut rest = sql.trim_start();
  loop {
    if let Some(after) = rest.strip_prefix("--") {
      rest = after
        .split_once('\n')
        .map_or("", |(_, tail)| tail)
        .trim_start();
    } else if let Some(after) = rest.strip_prefix("/*") {
      rest = after
        .split_once("*/")
        .map_or("", |(_, tail)| tail)
        .trim_start();
    } else {
      break;
    }
  }
  let end = rest
    .find(|c: char| !c.is_ascii_alphabetic())
    .unwrap_or(rest.len());
  &rest[..end]
}

fn contains_keyword(sql: &str, keyword: &str) -> bool {
  sql
    .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
    .any(|word| word.eq_ignore_ascii_case(keyword))
}
