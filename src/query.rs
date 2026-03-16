use serde_json::Value;

/// Sort direction for query ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
  Asc,
  Desc,
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
      Filter::And(filters) => filters.iter().all(|f| f.matches(doc)),
      Filter::Or(filters) => filters.iter().any(|f| f.matches(doc)),
      Filter::Not(inner) => !inner.matches(doc),
    }
  }
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

  /// Build the combined filter (AND of all accumulated conditions).
  pub fn build_filter(&self) -> Option<Filter> {
    match self.filters.len() {
      0 => None,
      1 => Some(self.filters[0].clone()),
      _ => Some(Filter::And(self.filters.clone())),
    }
  }
}
