use base64::{engine::general_purpose, Engine as _};

pub fn base64_encode(data: &[u8]) -> String {
  general_purpose::STANDARD.encode(data)
}

// because SQL statement classification is only consumed by the postgres adapter
#[cfg_attr(not(feature = "sql-postgres"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatementKind {
  Read,
  Write,
}

// because SQL statement classification is only consumed by the postgres adapter
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

// because the SQL Server and ClickHouse providers are the only crates that
// translate type names, and neither is part of the default build
#[cfg_attr(
  not(any(feature = "sql-clickhouse", feature = "sql-mssql")),
  allow(dead_code)
)]
pub(crate) fn replace_word(text: &str, from: &str, to: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(index) = rest.find(from) {
    let before = &rest[..index];
    let after = &rest[index + from.len()..];
    let boundary_before = before
      .chars()
      .last()
      .map(|c| !c.is_ascii_alphanumeric() && c != '_')
      .unwrap_or(true);
    let boundary_after = after
      .chars()
      .next()
      .map(|c| !c.is_ascii_alphanumeric() && c != '_')
      .unwrap_or(true);
    out.push_str(before);
    if boundary_before && boundary_after {
      out.push_str(to);
    } else {
      out.push_str(from);
    }
    rest = after;
  }
  out.push_str(rest);
  out
}

// because only the embedded/HTTP providers added for DuckDB, ClickHouse and
// SQL Server read counts out of JSON documents, not the default build
#[cfg_attr(
  not(any(
    feature = "sql-duckdb",
    feature = "sql-clickhouse",
    feature = "sql-mssql"
  )),
  allow(dead_code)
)]
pub(crate) fn value_as_u64(value: &serde_json::Value) -> Option<u64> {
  value
    .as_u64()
    .or_else(|| value.as_i64().map(|v| v as u64))
    .or_else(|| value.as_str().and_then(|s| s.parse::<u64>().ok()))
}

// The placeholder shapes of the two dialects whose bind syntax no builder
// in this crate emits.
// because the portable rewrite only has callers in the SQL Server and Oracle
// providers, which are not part of the default build
#[cfg_attr(
  not(any(feature = "sql-mssql", feature = "sql-oracle")),
  allow(dead_code)
)]
#[derive(Clone, Copy)]
pub(crate) enum PlaceholderStyle {
  // because the T-SQL shape only has callers when sql-mssql is enabled
  #[cfg_attr(not(feature = "sql-mssql"), allow(dead_code))]
  Tsql,
  // because the Oracle shape only has callers when sql-oracle is enabled
  #[cfg_attr(not(feature = "sql-oracle"), allow(dead_code))]
  Oracle,
}

impl PlaceholderStyle {
  fn placeholder(self, ordinal: usize) -> String {
    match self {
      PlaceholderStyle::Tsql => format!("@P{}", ordinal),
      PlaceholderStyle::Oracle => format!(":{}", ordinal),
    }
  }
}

// because the portable rewrite only has callers in the SQL Server and Oracle
// providers, which are not part of the default build
#[cfg_attr(
  not(any(feature = "sql-mssql", feature = "sql-oracle")),
  allow(dead_code)
)]
// Rewrites one statement for the two dialects that do not share the SQL
// the builders emit: `?` placeholders become `@P1…` (T-SQL) or `:1…`
// (Oracle), the TRUE/FALSE literals those dialects have no SQL meaning for
// become 1/0, and `LIMIT n OFFSET m` becomes the OFFSET/FETCH pair both
// understand. An ORDER BY is synthesized when FETCH needs one and the
// statement had none. String literals, quoted identifiers and bracketed
// identifiers are copied through untouched.
pub(crate) fn rewrite_portable_sql(sql: &str, style: PlaceholderStyle) -> String {
  let bytes = sql.as_bytes();
  let mut out = String::with_capacity(sql.len() + 16);
  let mut param_ordinal = 0usize;
  let mut saw_order_by = false;
  let mut tail: Option<(Option<usize>, Option<usize>)> = None;
  let mut index = 0usize;

  while index < bytes.len() {
    let byte = bytes[index];

    if byte == b'\'' {
      index = copy_quoted(&mut out, sql, index, b'\'');
      continue;
    }
    if byte == b'"' {
      index = copy_quoted(&mut out, sql, index, b'"');
      continue;
    }
    if byte == b'[' {
      index = copy_bracket(&mut out, sql, index);
      continue;
    }
    if byte == b'?' {
      param_ordinal += 1;
      out.push_str(&style.placeholder(param_ordinal));
      index += 1;
      continue;
    }
    if byte.is_ascii_alphabetic() {
      let start = index;
      while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
      }
      let word = &sql[start..index];

      if word.eq_ignore_ascii_case("TRUE") {
        out.push('1');
        continue;
      }
      if word.eq_ignore_ascii_case("FALSE") {
        out.push('0');
        continue;
      }
      if word.eq_ignore_ascii_case("ORDER") {
        if word_after(sql, index, "BY").is_some() {
          saw_order_by = true;
        }
        out.push_str(word);
        continue;
      }
      if word.eq_ignore_ascii_case("LIMIT") {
        let (value, next) = read_ordinal(sql, index);
        let mut offset = None;
        if let Some(next_offset) = word_after(sql, next, "OFFSET") {
          let (value_offset, after_offset) = read_ordinal(sql, next_offset);
          offset = value_offset;
          index = after_offset;
        } else {
          index = next;
        }
        tail = Some((value, offset));
        continue;
      }
      if word.eq_ignore_ascii_case("OFFSET") {
        let (value, next) = read_ordinal(sql, index);
        if value.is_some() {
          tail = Some((None, value));
          index = next;
          continue;
        }
        out.push_str(word);
        continue;
      }

      out.push_str(word);
      continue;
    }

    match std::str::from_utf8(&bytes[index..])
      .ok()
      .and_then(|s| s.chars().next())
    {
      Some(ch) => {
        out.push(ch);
        index += ch.len_utf8();
      }
      None => break,
    }
  }

  if let Some((limit, offset)) = tail {
    let mut parts: Vec<String> = Vec::new();
    if !saw_order_by {
      // OFFSET/FETCH requires an ORDER BY; without one the result order was
      // arbitrary anyway, so a constant order keeps the clause legal.
      parts.push("ORDER BY (SELECT NULL)".to_string());
    }
    match (limit, offset) {
      (Some(limit_value), Some(offset_value)) => {
        parts.push(format!(
          "OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
          offset_value, limit_value
        ));
      }
      (Some(limit_value), None) => {
        parts.push(format!(
          "OFFSET 0 ROWS FETCH NEXT {} ROWS ONLY",
          limit_value
        ));
      }
      (None, Some(offset_value)) => {
        parts.push(format!("OFFSET {} ROWS", offset_value));
      }
      (None, None) => {}
    }
    if !parts.is_empty() {
      if !out.is_empty() && !out.ends_with(char::is_whitespace) {
        out.push(' ');
      }
      out.push_str(&parts.join(" "));
    }
  }

  out
}

// Copies a `'…'` or `"…"` span (with its doubled-quote escapes) and returns
// the index just past it.
fn copy_quoted(out: &mut String, sql: &str, start: usize, quote: u8) -> usize {
  let bytes = sql.as_bytes();
  let mut index = start;
  out.push(sql[start..start + 1].chars().next().unwrap_or(' '));
  index += 1;
  while index < bytes.len() {
    if bytes[index] == quote {
      if index + 1 < bytes.len() && bytes[index + 1] == quote {
        out.push_str(&sql[index..index + 2]);
        index += 2;
        continue;
      }
      out.push(sql[index..index + 1].chars().next().unwrap_or(' '));
      return index + 1;
    }
    let ch = match sql[index..].chars().next() {
      Some(ch) => ch,
      None => break,
    };
    out.push(ch);
    index += ch.len_utf8();
  }
  index
}

// Copies a `[…]` span (with `]]` as an escaped bracket).
fn copy_bracket(out: &mut String, sql: &str, start: usize) -> usize {
  let bytes = sql.as_bytes();
  let mut index = start;
  out.push('[');
  index += 1;
  while index < bytes.len() {
    if bytes[index] == b']' {
      if index + 1 < bytes.len() && bytes[index + 1] == b']' {
        out.push_str("]]");
        index += 2;
        continue;
      }
      out.push(']');
      return index + 1;
    }
    let ch = match sql[index..].chars().next() {
      Some(ch) => ch,
      None => break,
    };
    out.push(ch);
    index += ch.len_utf8();
  }
  index
}

// Skips whitespace at `from` and, if the next word is `word`, returns the
// index just past it.
fn word_after(sql: &str, from: usize, word: &str) -> Option<usize> {
  let bytes = sql.as_bytes();
  let mut index = from;
  while index < bytes.len() && bytes[index].is_ascii_whitespace() {
    index += 1;
  }
  let end = index + word.len();
  if end <= bytes.len() && sql[index..end].eq_ignore_ascii_case(word) {
    return Some(end);
  }
  None
}

// Reads the digits at `from` (after optional space), returning the value and
// the index just past the number.
fn read_ordinal(sql: &str, from: usize) -> (Option<usize>, usize) {
  let bytes = sql.as_bytes();
  let mut index = from;
  while index < bytes.len() && bytes[index].is_ascii_whitespace() {
    index += 1;
  }
  let start = index;
  while index < bytes.len() && bytes[index].is_ascii_digit() {
    index += 1;
  }
  if start == index {
    return (None, index);
  }
  let value = sql[start..index].parse::<usize>().ok();
  (value, index)
}
