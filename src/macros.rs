use crate::query::Filter;
use serde_json::Value;

#[macro_export]
macro_rules! filter {
  ($field:expr, eq, $value:expr) => {
    Filter::Eq($field.to_string(), serde_json::json!($value))
  };
  ($field:expr, ne, $value:expr) => {
    Filter::Ne($field.to_string(), serde_json::json!($value))
  };
  ($field:expr, gt, $value:expr) => {
    Filter::Gt($field.to_string(), serde_json::json!($value))
  };
  ($field:expr, gte, $value:expr) => {
    Filter::Gte($field.to_string(), serde_json::json!($value))
  };
  ($field:expr, lt, $value:expr) => {
    Filter::Lt($field.to_string(), serde_json::json!($value))
  };
  ($field:expr, lte, $value:expr) => {
    Filter::Lte($field.to_string(), serde_json::json!($value))
  };
  ($field:expr, in, [$($values:expr),*]) => {
    Filter::In(
      $field.to_string(),
      vec![$(serde_json::json!($values)),*]
    )
  };
  ($field:expr, like, $value:expr) => {
    Filter::Like($field.to_string(), serde_json::json!($value).as_str().unwrap_or("").to_string())
  };
}

#[macro_export]
macro_rules! filters {
  ($($field:expr, $op:tt, $value:expr),* $(,)?) => {
    {
      let mut conditions = Vec::new();
      $(
        conditions.push(filter!($field, $op, $value));
      )*
      if conditions.len() == 1 {
        conditions.pop().unwrap()
      } else {
        Filter::And(conditions)
      }
    }
  };
}

pub fn eq_filter(field: &str, value: impl Into<Value>) -> Filter {
  Filter::Eq(field.to_string(), value.into())
}

pub fn ne_filter(field: &str, value: impl Into<Value>) -> Filter {
  Filter::Ne(field.to_string(), value.into())
}

pub fn gt_filter(field: &str, value: impl Into<Value>) -> Filter {
  Filter::Gt(field.to_string(), value.into())
}

pub fn gte_filter(field: &str, value: impl Into<Value>) -> Filter {
  Filter::Gte(field.to_string(), value.into())
}

pub fn lt_filter(field: &str, value: impl Into<Value>) -> Filter {
  Filter::Lt(field.to_string(), value.into())
}

pub fn lte_filter(field: &str, value: impl Into<Value>) -> Filter {
  Filter::Lte(field.to_string(), value.into())
}

pub fn in_filter(field: &str, values: Vec<Value>) -> Filter {
  Filter::In(field.to_string(), values)
}

pub fn and_filter(filters: Vec<Filter>) -> Filter {
  Filter::And(filters)
}

pub fn or_filter(filters: Vec<Filter>) -> Filter {
  Filter::Or(filters)
}

pub fn not_filter(filter: Filter) -> Filter {
  Filter::Not(Box::new(filter))
}

pub fn is_null_filter(field: &str) -> Filter {
  Filter::IsNull(field.to_string())
}

pub fn is_not_null_filter(field: &str) -> Filter {
  Filter::IsNotNull(field.to_string())
}
