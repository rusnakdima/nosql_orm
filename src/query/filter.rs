use crate::error::{OrmError, OrmResult};
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum Filter {
  Eq(String, Value),
  Ne(String, Value),
  Gt(String, Value),
  Gte(String, Value),
  Lt(String, Value),
  Lte(String, Value),
  In(String, Vec<Value>),
  NotIn(String, Vec<Value>),
  ArrayContains(String, Value),
  Contains(String, String),
  StartsWith(String, String),
  EndsWith(String, String),
  Like(String, String),
  IsNull(String),
  IsNotNull(String),
  Between(String, Value, Value),
  And(Vec<Filter>),
  Or(Vec<Filter>),
  Not(Box<Filter>),
}

impl Filter {
  pub fn matches(&self, doc: &Value) -> bool {
    match self {
      Filter::Eq(field, val) => get_field(doc, field) == Some(val),
      Filter::Ne(field, val) => get_field(doc, field) != Some(val),
      Filter::Gt(field, val) => compare(doc, field, val, |o| o.is_gt()),
      Filter::Gte(field, val) => compare(doc, field, val, |o| o.is_ge()),
      Filter::Lt(field, val) => compare(doc, field, val, |o| o.is_lt()),
      Filter::Lte(field, val) => compare(doc, field, val, |o| o.is_le()),
      Filter::In(field, vals) => get_field(doc, field).is_some_and(|v| vals.contains(v)),
      Filter::NotIn(field, vals) => get_field(doc, field).is_none_or(|v| !vals.contains(v)),
      Filter::ArrayContains(field, val) => get_field(doc, field)
        .and_then(|v| v.as_array())
        .map(|arr| arr.contains(val))
        .unwrap_or(false),
      Filter::Contains(field, sub) => get_field(doc, field)
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.to_lowercase().contains(&sub.to_lowercase())),
      Filter::StartsWith(field, prefix) => get_field(doc, field)
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.to_lowercase().starts_with(&prefix.to_lowercase())),
      Filter::EndsWith(field, suffix) => get_field(doc, field)
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.to_lowercase().ends_with(&suffix.to_lowercase())),
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
      Filter::IsNull(field) => get_field(doc, field).is_some_and(|v| v.is_null()),
      Filter::IsNotNull(field) => get_field(doc, field).is_some_and(|v| !v.is_null()),
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
        let mut or_filters: Option<Vec<Filter>> = None;
        let mut not_filter: Option<Filter> = None;
        for (key, val) in obj {
          match key.as_str() {
            "$and" => {
              if let Value::Array(arr) = val {
                for item in arr {
                  filters.push(Filter::from_json(item)?);
                }
              }
            }
            "$or" => {
              if let Value::Array(arr) = val {
                let or_items: OrmResult<Vec<Filter>> = arr.iter().map(Filter::from_json).collect();
                or_filters = Some(or_items?);
              }
            }
            "$not" => {
              not_filter = Some(Filter::from_json(val)?);
            }
            _ => {
              filters.push(parse_field_filter(key, val)?);
            }
          }
        }
        if let Some(or_items) = or_filters {
          filters.push(Filter::Or(or_items));
        }
        if let Some(not_item) = not_filter {
          filters.push(Filter::Not(Box::new(not_item)));
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

pub fn parse_field_filter(field: &str, value: &Value) -> OrmResult<Filter> {
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
            "$arrayContains" => {
              return Ok(Filter::ArrayContains(field.to_string(), val.clone()));
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
            _ => {
              return Err(OrmError::InvalidInput(format!(
                "Unhandled filter operator '{}'",
                op
              )))
            }
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
      af.partial_cmp(&bf).is_some_and(check)
    }
    (Value::String(a), Value::String(b)) => a.partial_cmp(b).is_some_and(check),
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
      af.partial_cmp(&bf).is_some_and(check)
    }
    (Value::String(a), Value::String(b)) => a.partial_cmp(b).is_some_and(check),
    _ => false,
  }
}
