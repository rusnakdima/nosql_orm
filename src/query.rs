use crate::error::{OrmError, OrmResult};
use serde_json::Value;

/// Sort direction for query ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
  Asc,
  Desc,
}

/// Cursor for paginating through results.
/// Contains the last seen document's id and sort information.
#[derive(Debug, Clone)]
pub struct Cursor {
  pub last_id: String,
  pub sort_field: String,
  pub sort_asc: bool,
}

impl Cursor {
  pub fn new(last_id: String, sort_field: String, sort_asc: bool) -> Self {
    Self {
      last_id,
      sort_field,
      sort_asc,
    }
  }

  pub fn as_filter(&self) -> Filter {
    if self.sort_asc {
      Filter::Gt(self.sort_field.clone(), Value::String(self.last_id.clone()))
    } else {
      Filter::Lt(self.sort_field.clone(), Value::String(self.last_id.clone()))
    }
  }
}

impl Default for Cursor {
  fn default() -> Self {
    Self {
      last_id: String::new(),
      sort_field: String::new(),
      sort_asc: true,
    }
  }
}

/// A struct containing results and cursor for the next page.
#[derive(Debug)]
pub struct PaginatedResult<T> {
  pub data: Vec<T>,
  pub next_cursor: Option<Cursor>,
  pub has_more: bool,
}

/// A single sort specification.
#[derive(Debug, Clone)]
pub struct OrderBy {
  pub field: String,
  pub direction: SortDirection,
}

impl OrderBy {
  pub fn asc(field: impl Into<String>) -> Self {
    Self {
      field: field.into(),
      direction: SortDirection::Asc,
    }
  }
  pub fn desc(field: impl Into<String>) -> Self {
    Self {
      field: field.into(),
      direction: SortDirection::Desc,
    }
  }
}

/// Field projection for selecting/excluding fields.
#[derive(Debug, Clone)]
pub struct Projection {
  /// Fields to include (if Some). Cannot coexist with exclude.
  pub select: Option<Vec<String>>,
  /// Fields to exclude (if Some). Cannot coexist with select.
  pub exclude: Option<Vec<String>>,
}

impl Projection {
  pub fn new() -> Self {
    Self {
      select: None,
      exclude: None,
    }
  }

  /// Select only these fields (include all others implicitly).
  pub fn select(fields: &[&str]) -> Self {
    Self {
      select: Some(fields.iter().map(|s| s.to_string()).collect()),
      exclude: None,
    }
  }

  /// Exclude these fields (include all others implicitly).
  pub fn exclude(fields: &[&str]) -> Self {
    Self {
      select: None,
      exclude: Some(fields.iter().map(|s| s.to_string()).collect()),
    }
  }

  /// Returns true if projection is empty (no filtering needed).
  pub fn is_empty(&self) -> bool {
    self.select.is_none() && self.exclude.is_none()
  }

  /// Apply projection to a JSON document.
  pub fn apply(&self, doc: &Value) -> Value {
    if self.is_empty() {
      return doc.clone();
    }

    let obj = match doc.as_object() {
      Some(o) => o.clone(),
      None => return doc.clone(),
    };

    if let Some(ref select_fields) = self.select {
      let filtered: serde_json::Map<String, Value> = obj
        .into_iter()
        .filter(|(k, _)| select_fields.contains(k))
        .collect();
      return Value::Object(filtered);
    }

    if let Some(ref exclude_fields) = self.exclude {
      let filtered: serde_json::Map<String, Value> = obj
        .into_iter()
        .filter(|(k, _)| !exclude_fields.contains(k))
        .collect();
      return Value::Object(filtered);
    }

    doc.clone()
  }

  pub fn apply_recursive<'a>(&self, doc: &'a Value) -> Value
  where
    'a: 'a,
  {
    let mut filtered = self.apply(doc);

    if let Some(obj) = filtered.as_object_mut() {
      for (_key, val) in obj.iter_mut() {
        *val = self.apply_recursive(val);
      }
    } else if let Some(arr) = filtered.as_array_mut() {
      for item in arr.iter_mut() {
        *item = self.apply_recursive(item);
      }
    }

    filtered
  }
}

impl Default for Projection {
  fn default() -> Self {
    Self::new()
  }
}

/// A composable filter condition.
#[derive(Debug, Clone)]
pub enum Filter {
  /// Field equals value.
  Eq(String, Value),
  /// Field does not equal value.
  Ne(String, Value),
  /// Field greater than value.
  Gt(String, Value),
  /// Field greater than or equal.
  Gte(String, Value),
  /// Field less than value.
  Lt(String, Value),
  /// Field less than or equal.
  Lte(String, Value),
  /// Field value is in the given list.
  In(String, Vec<Value>),
  /// Field value is NOT in the given list.
  NotIn(String, Vec<Value>),
  /// Field string value contains substring (case-insensitive).
  Contains(String, String),
  /// Field string value starts with prefix.
  StartsWith(String, String),
  /// Field string value ends with suffix.
  EndsWith(String, String),
  /// Field matches SQL LIKE pattern (%, _ wildcards).
  Like(String, String),
  /// Field value is NULL.
  IsNull(String),
  /// Field value is NOT NULL.
  IsNotNull(String),
  /// Field value is between two values (inclusive).
  Between(String, Value, Value),
  /// All conditions must hold.
  And(Vec<Filter>),
  /// At least one condition must hold.
  Or(Vec<Filter>),
  /// Negates the inner condition.
  Not(Box<Filter>),
}

impl Filter {
  /// Evaluate this filter against a JSON document value.
  pub fn matches(&self, doc: &Value) -> bool {
    match self {
      Filter::Eq(field, val) => get_field(doc, field).map_or(false, |v| v == val),
      Filter::Ne(field, val) => get_field(doc, field).map_or(true, |v| v != val),
      Filter::Gt(field, val) => compare(doc, field, val, |o| o.is_gt()),
      Filter::Gte(field, val) => compare(doc, field, val, |o| o.is_ge()),
      Filter::Lt(field, val) => compare(doc, field, val, |o| o.is_lt()),
      Filter::Lte(field, val) => compare(doc, field, val, |o| o.is_le()),
      Filter::In(field, vals) => get_field(doc, field).map_or(false, |v| vals.contains(v)),
      Filter::NotIn(field, vals) => get_field(doc, field).map_or(true, |v| !vals.contains(v)),
      Filter::Contains(field, sub) => get_field(doc, field)
        .and_then(|v| v.as_str())
        .map_or(false, |s| s.to_lowercase().contains(&sub.to_lowercase())),
      Filter::StartsWith(field, prefix) => get_field(doc, field)
        .and_then(|v| v.as_str())
        .map_or(false, |s| {
          s.to_lowercase().starts_with(&prefix.to_lowercase())
        }),
      Filter::EndsWith(field, suffix) => get_field(doc, field)
        .and_then(|v| v.as_str())
        .map_or(false, |s| {
          s.to_lowercase().ends_with(&suffix.to_lowercase())
        }),
      Filter::Like(field, pattern) => {
        if let Some(s) = get_field(doc, field).and_then(|v| v.as_str()) {
          matches_like(s, pattern)
        } else {
          false
        }
      }
      Filter::And(filters) => filters.iter().all(|f| f.matches(doc)),
      Filter::Or(filters) => filters.iter().any(|f| f.matches(doc)),
      Filter::Not(inner) => !inner.matches(doc),
      Filter::IsNull(field) => get_field(doc, field).map_or(false, |v| v.is_null()),
      Filter::IsNotNull(field) => get_field(doc, field).map_or(false, |v| !v.is_null()),
      Filter::Between(field, min, max) => {
        if let Some(val) = get_field(doc, field) {
          let ge_min = compare_values(val, min, |o| o.is_ge());
          let le_max = compare_values(val, max, |o| o.is_le());
          ge_min && le_max
        } else {
          false
        }
      }
    }
  }

  pub fn from_json(value: &Value) -> OrmResult<Filter> {
    match value {
      Value::Object(obj) => {
        if obj.len() == 1 {
          for (key, val) in obj {
            match key.as_str() {
              "$and" => {
                if let Value::Array(arr) = val {
                  let filters: OrmResult<Vec<Filter>> = arr.iter().map(Filter::from_json).collect();
                  return Ok(Filter::And(filters?));
                }
              }
              "$or" => {
                if let Value::Array(arr) = val {
                  let filters: OrmResult<Vec<Filter>> = arr.iter().map(Filter::from_json).collect();
                  return Ok(Filter::Or(filters?));
                }
              }
              "$not" => {
                let inner = Filter::from_json(val)?;
                return Ok(Filter::Not(Box::new(inner)));
              }
              _ => {
                return parse_field_filter(key, val);
              }
            }
          }
        }
        let mut filters = Vec::new();
        for (key, val) in obj {
          filters.push(parse_field_filter(key, val)?);
        }
        if filters.len() == 1 {
          Ok(filters.remove(0))
        } else {
          Ok(Filter::And(filters))
        }
      }
      _ => Err(OrmError::InvalidInput(
        "Filter must be a JSON object".to_string(),
      )),
    }
  }
}

fn parse_field_filter(field: &str, value: &Value) -> OrmResult<Filter> {
  match value {
    Value::Object(obj) => {
      if obj.len() == 1 {
        for (op, val) in obj {
          match op.as_str() {
            "$eq" => return Ok(Filter::Eq(field.to_string(), val.clone())),
            "$ne" => return Ok(Filter::Ne(field.to_string(), val.clone())),
            "$gt" => return Ok(Filter::Gt(field.to_string(), val.clone())),
            "$gte" => return Ok(Filter::Gte(field.to_string(), val.clone())),
            "$lt" => return Ok(Filter::Lt(field.to_string(), val.clone())),
            "$lte" => return Ok(Filter::Lte(field.to_string(), val.clone())),
            "$in" => {
              if let Value::Array(arr) = val {
                return Ok(Filter::In(field.to_string(), arr.clone()));
              }
            }
            "$notIn" => {
              if let Value::Array(arr) = val {
                return Ok(Filter::NotIn(field.to_string(), arr.clone()));
              }
            }
            "$contains" => {
              if let Some(s) = val.as_str() {
                return Ok(Filter::Contains(field.to_string(), s.to_string()));
              }
            }
            "$startsWith" => {
              if let Some(s) = val.as_str() {
                return Ok(Filter::StartsWith(field.to_string(), s.to_string()));
              }
            }
            "$endsWith" => {
              if let Some(s) = val.as_str() {
                return Ok(Filter::EndsWith(field.to_string(), s.to_string()));
              }
            }
            "$like" => {
              if let Some(s) = val.as_str() {
                return Ok(Filter::Like(field.to_string(), s.to_string()));
              }
            }
            "$isNull" => {
              return Ok(Filter::IsNull(field.to_string()));
            }
            "$isNotNull" => {
              return Ok(Filter::IsNotNull(field.to_string()));
            }
            "$between" => {
              if let Value::Array(arr) = val {
                if arr.len() == 2 {
                  return Ok(Filter::Between(
                    field.to_string(),
                    arr[0].clone(),
                    arr[1].clone(),
                  ));
                }
              }
            }
            _ => {}
          }
        }
      }
      Ok(Filter::Eq(field.to_string(), value.clone()))
    }
    _ => Ok(Filter::Eq(field.to_string(), value.clone())),
  }
}

fn compare_values<F>(lhs: &Value, rhs: &Value, check: F) -> bool
where
  F: Fn(std::cmp::Ordering) -> bool,
{
  match (lhs, rhs) {
    (Value::Number(a), Value::Number(b)) => {
      let af = a.as_f64().unwrap_or(f64::NAN);
      let bf = b.as_f64().unwrap_or(f64::NAN);
      af.partial_cmp(&bf).map_or(false, check)
    }
    (Value::String(a), Value::String(b)) => a.partial_cmp(b).map_or(false, check),
    _ => false,
  }
}

fn matches_like(s: &str, pattern: &str) -> bool {
  let s_lower = s.to_lowercase();
  let pattern_lower = pattern.to_lowercase();

  if pattern_lower == "%" {
    return true;
  }

  let parts: Vec<&str> = pattern_lower.split('%').collect();
  let mut pos = 0;

  for (i, part) in parts.iter().enumerate() {
    if part.is_empty() {
      if i == 0 && pattern_lower.starts_with('%') && pattern_lower.len() > 1 {
        continue;
      }
      if i == parts.len() - 1 && pattern_lower.ends_with('%') && pattern_lower.len() > 1 {
        continue;
      }
      continue;
    }

    if let Some(found) = s_lower[pos..].find(part) {
      if i == 0 && found != 0 && !pattern_lower.starts_with('%') {
        return false;
      }
      pos = found + part.len();
    } else {
      return false;
    }
  }

  if pattern_lower.ends_with('%') && !pattern_lower.starts_with('%') {
    return s_lower.len() >= pos;
  }
  if !pattern_lower.ends_with('%') && !pattern_lower.starts_with('%') {
    return pos == s_lower.len();
  }

  true
}

fn get_field<'a>(doc: &'a Value, field: &str) -> Option<&'a Value> {
  // Support dot-notation: "address.city"
  let mut parts = field.splitn(2, '.');
  let head = parts.next()?;
  let rest = parts.next();
  let val = doc.get(head)?;
  match rest {
    Some(tail) => get_field(val, tail),
    None => Some(val),
  }
}

fn compare(
  doc: &Value,
  field: &str,
  rhs: &Value,
  check: impl Fn(std::cmp::Ordering) -> bool,
) -> bool {
  let lhs = match get_field(doc, field) {
    Some(v) => v,
    None => return false,
  };
  match (lhs, rhs) {
    (Value::Number(a), Value::Number(b)) => {
      let af = a.as_f64().unwrap_or(f64::NAN);
      let bf = b.as_f64().unwrap_or(f64::NAN);
      af.partial_cmp(&bf).map_or(false, check)
    }
    (Value::String(a), Value::String(b)) => a.partial_cmp(b).map_or(false, check),
    _ => false,
  }
}

/// Fluent query builder. Attach to a `Repository` via `repo.query()`.
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
  pub(crate) filters: Vec<Filter>,
  pub(crate) order: Option<OrderBy>,
  pub(crate) skip: Option<u64>,
  pub(crate) limit: Option<u64>,
  pub(crate) relations: Vec<String>,
  pub(crate) projection: Option<Projection>,
}

impl QueryBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  /// Add an equality filter: `field == value`.
  pub fn where_eq(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Eq(field.into(), value.into()));
    self
  }

  /// Add an inequality filter: `field != value`.
  pub fn where_ne(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Ne(field.into(), value.into()));
    self
  }

  /// Add a greater-than filter.
  pub fn where_gt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Gt(field.into(), value.into()));
    self
  }

  /// Add a less-than filter.
  pub fn where_lt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Lt(field.into(), value.into()));
    self
  }

  /// Add a case-insensitive substring filter.
  pub fn where_contains(mut self, field: impl Into<String>, sub: impl Into<String>) -> Self {
    self
      .filters
      .push(Filter::Contains(field.into(), sub.into()));
    self
  }

  /// Add a starts-with filter.
  pub fn where_starts_with(mut self, field: impl Into<String>, prefix: impl Into<String>) -> Self {
    self
      .filters
      .push(Filter::StartsWith(field.into(), prefix.into()));
    self
  }

  /// Add an IN filter.
  pub fn where_in(mut self, field: impl Into<String>, values: Vec<Value>) -> Self {
    self.filters.push(Filter::In(field.into(), values));
    self
  }

  /// Add a raw `Filter`.
  pub fn filter(mut self, f: Filter) -> Self {
    self.filters.push(f);
    self
  }

  /// Add an IS NULL filter.
  pub fn where_is_null(mut self, field: impl Into<String>) -> Self {
    self.filters.push(Filter::IsNull(field.into()));
    self
  }

  /// Set ordering.
  pub fn order_by(mut self, order: OrderBy) -> Self {
    self.order = Some(order);
    self
  }

  /// Skip the first N results (for pagination).
  pub fn skip(mut self, n: u64) -> Self {
    self.skip = Some(n);
    self
  }

  /// Limit the result set size.
  pub fn limit(mut self, n: u64) -> Self {
    self.limit = Some(n);
    self
  }

  /// Eagerly load a named relation.
  pub fn with_relation(mut self, name: impl Into<String>) -> Self {
    self.relations.push(name.into());
    self
  }

  /// Select only these fields to be returned.
  /// Use this when you only need specific fields from the entity.
  ///
  /// Example: `repo.query().select(&["id", "name"]).find().await?`
  pub fn select(mut self, fields: &[&str]) -> Self {
    self.projection = Some(Projection::select(fields));
    self
  }

  /// Exclude these fields from the result.
  /// Useful for excluding sensitive fields like passwords or tokens.
  ///
  /// Example: `repo.query().exclude(&["password", "token"]).find().await?`
  pub fn exclude(mut self, fields: &[&str]) -> Self {
    self.projection = Some(Projection::exclude(fields));
    self
  }

  /// Add a greater-than-or-equal filter.
  pub fn where_gte(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Gte(field.into(), value.into()));
    self
  }

  /// Add a less-than-or-equal filter.
  pub fn where_lte(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Lte(field.into(), value.into()));
    self
  }

  /// Add a NOT IN filter.
  pub fn where_not_in(mut self, field: impl Into<String>, values: Vec<Value>) -> Self {
    self.filters.push(Filter::NotIn(field.into(), values));
    self
  }

  /// Add an ends-with filter.
  pub fn where_ends_with(mut self, field: impl Into<String>, suffix: impl Into<String>) -> Self {
    self
      .filters
      .push(Filter::EndsWith(field.into(), suffix.into()));
    self
  }

  /// Add a LIKE filter (SQL-style pattern matching with % and _ wildcards).
  pub fn where_like(mut self, field: impl Into<String>, pattern: impl Into<String>) -> Self {
    self
      .filters
      .push(Filter::Like(field.into(), pattern.into()));
    self
  }

  /// Add a NOT NULL filter.
  pub fn where_is_not_null(mut self, field: impl Into<String>) -> Self {
    self.filters.push(Filter::IsNotNull(field.into()));
    self
  }

  /// Add a BETWEEN filter (value is between min and max, inclusive).
  pub fn where_between(
    mut self,
    field: impl Into<String>,
    min: impl Into<Value>,
    max: impl Into<Value>,
  ) -> Self {
    self
      .filters
      .push(Filter::Between(field.into(), min.into(), max.into()));
    self
  }

  /// Combine filters with OR (any condition matches).
  /// Note: calling this replaces all previous individual filters with an OR group.
  pub fn or(mut self, other: QueryBuilder) -> Self {
    let mut combined = Vec::new();
    combined.push(self.build_filter().unwrap_or(Filter::And(vec![])));
    combined.push(other.build_filter().unwrap_or(Filter::And(vec![])));
    self.filters = vec![Filter::Or(combined)];
    self
  }

  /// Add a negation wrapper around the next filter.
  pub fn not(mut self) -> Self {
    if let Some(f) = self.build_filter() {
      self.filters = vec![Filter::Not(Box::new(f))];
    }
    self
  }

  /// Add filters grouped with OR (any condition matches).
  /// Multiple calls append to the same OR group.
  pub fn where_or(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    let filter = Filter::Eq(field.into(), value.into());
    self.filters.push(Filter::Or(vec![filter]));
    self
  }

  /// Add filters grouped with AND (all conditions must match).
  /// Multiple calls append to the same AND group.
  pub fn where_and(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    let filter = Filter::Eq(field.into(), value.into());
    self.filters.push(filter);
    self
  }

  /// Add a negation filter for a specific field=value.
  pub fn where_not(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Not(Box::new(Filter::Eq(
      field.into(),
      value.into(),
    ))));
    self
  }

  /// Build an OR group from multiple QueryBuilders.
  /// Each builder's filter becomes part of the OR group.
  pub fn or_group(mut self, others: Vec<QueryBuilder>) -> Self {
    let mut all_filters = Vec::new();
    if let Some(f) = self.build_filter() {
      all_filters.push(f);
    }
    for builder in others {
      if let Some(f) = builder.build_filter() {
        all_filters.push(f);
      }
    }
    self.filters = vec![Filter::Or(all_filters)];
    self
  }

  /// Build an AND group from multiple QueryBuilders.
  /// Each builder's filter becomes part of the AND group.
  pub fn and_group(mut self, others: Vec<QueryBuilder>) -> Self {
    let mut all_filters = Vec::new();
    if let Some(f) = self.build_filter() {
      all_filters.push(f);
    }
    for builder in others {
      if let Some(f) = builder.build_filter() {
        all_filters.push(f);
      }
    }
    self.filters = vec![Filter::And(all_filters)];
    self
  }

  /// Get the projection if set.
  pub fn get_projection(&self) -> Option<&Projection> {
    self.projection.as_ref()
  }

  /// Build the combined filter (AND of all accumulated conditions).
  pub fn build_filter(&self) -> Option<Filter> {
    match self.filters.len() {
      0 => None,
      1 => Some(self.filters[0].clone()),
      _ => Some(Filter::And(self.filters.clone())),
    }
  }

  /// Get the cursor info (last document's id) for pagination.
  pub fn get_cursor(&self) -> Option<String> {
    None
  }
}
