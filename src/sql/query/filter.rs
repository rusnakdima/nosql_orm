use crate::error::{OrmError, OrmResult};
use crate::query::Filter;
use serde_json::Value;

use super::builder::SqlQueryBuilder;

fn validate_identifier(name: &str) -> OrmResult<()> {
  if name.is_empty() {
    return Err(OrmError::InvalidInput(
      "Identifier cannot be empty".to_string(),
    ));
  }
  if name.len() > 64 {
    return Err(OrmError::InvalidInput(
      "Identifier exceeds maximum length of 64".to_string(),
    ));
  }
  if name.contains('\0') {
    return Err(OrmError::InvalidInput(format!(
      "Identifier '{}' contains a NUL byte",
      name
    )));
  }
  Ok(())
}

impl SqlQueryBuilder {
  // Returns the filter as a literal SQL string. Kept for callers that only
  // need to display or compare a statement (build_select, explain paths). The
  // providers execute filter_to_sql_params instead; executing this form with
  // untrusted values is unsafe.
  pub fn filter_to_sql(&self, filter: &Filter) -> OrmResult<String> {
    match filter {
      Filter::Eq(field, value) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} = {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        ))
      }
      Filter::Ne(field, value) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} <> {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        ))
      }
      Filter::Gt(field, value) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} > {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        ))
      }
      Filter::Gte(field, value) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} >= {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        ))
      }
      Filter::Lt(field, value) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} < {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        ))
      }
      Filter::Lte(field, value) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} <= {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        ))
      }
      Filter::In(field, values) => {
        validate_identifier(field)?;
        let values_str = values
          .iter()
          .map(|v| self.value_to_sql(v))
          .collect::<Vec<_>>()
          .join(", ");
        Ok(format!(
          "{} IN ({})",
          self.dialect().quote_identifier(field),
          values_str
        ))
      }
      Filter::NotIn(field, values) => {
        validate_identifier(field)?;
        let values_str = values
          .iter()
          .map(|v| self.value_to_sql(v))
          .collect::<Vec<_>>()
          .join(", ");
        Ok(format!(
          "{} NOT IN ({})",
          self.dialect().quote_identifier(field),
          values_str
        ))
      }
      Filter::ArrayContains(field, value) => {
        validate_identifier(field)?;
        Ok(format!(
          "JSON_CONTAINS({}, {})",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        ))
      }
      Filter::Contains(field, value) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(&serde_json::json!(format!("%{}%", value)))
        ))
      }
      Filter::StartsWith(field, prefix) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(&serde_json::json!(format!("{}%", prefix)))
        ))
      }
      Filter::IsNull(field) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} IS NULL",
          self.dialect().quote_identifier(field)
        ))
      }
      Filter::IsNotNull(field) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} IS NOT NULL",
          self.dialect().quote_identifier(field)
        ))
      }
      Filter::Like(field, pattern) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(&serde_json::json!(pattern))
        ))
      }
      Filter::EndsWith(field, suffix) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(&serde_json::json!(format!("%{}", suffix)))
        ))
      }
      Filter::Between(field, min, max) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} BETWEEN {} AND {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(min),
          self.value_to_sql(max)
        ))
      }
      Filter::And(filters) => {
        let mut strs = Vec::new();
        for f in filters {
          strs.push(format!("({})", self.filter_to_sql(f)?));
        }
        Ok(strs.join(" AND "))
      }
      Filter::Or(filters) => {
        let mut strs = Vec::new();
        for f in filters {
          strs.push(format!("({})", self.filter_to_sql(f)?));
        }
        Ok(strs.join(" OR "))
      }
      Filter::Not(inner) => Ok(format!("NOT ({})", self.filter_to_sql(inner)?)),
    }
  }

  // The bound form of filter_to_sql: placeholders in the statement and the
  // values in the order the placeholders expect. This is what the SQL
  // providers execute, so a filter value never becomes SQL text.
  pub fn filter_to_sql_params(&self, filter: &Filter) -> OrmResult<(String, Vec<Value>)> {
    let mut params = Vec::new();
    let sql = self.filter_to_bound_sql(filter, &mut params)?;
    Ok((sql, params))
  }

  fn push_param(&self, params: &mut Vec<Value>, value: Value) -> String {
    let placeholder = self.dialect().parameter_placeholder(params.len());
    params.push(value);
    placeholder
  }

  fn filter_to_bound_sql(
    &self,
    filter: &Filter,
    params: &mut Vec<Value>,
  ) -> OrmResult<String> {
    match filter {
      Filter::Eq(field, value) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, value.clone());
        Ok(format!(
          "{} = {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::Ne(field, value) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, value.clone());
        Ok(format!(
          "{} <> {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::Gt(field, value) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, value.clone());
        Ok(format!(
          "{} > {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::Gte(field, value) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, value.clone());
        Ok(format!(
          "{} >= {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::Lt(field, value) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, value.clone());
        Ok(format!(
          "{} < {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::Lte(field, value) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, value.clone());
        Ok(format!(
          "{} <= {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::In(field, values) => {
        validate_identifier(field)?;
        if values.is_empty() {
          // An empty IN list matches nothing; SQL has no valid empty ().
          return Ok("1 = 0".to_string());
        }
        let placeholders: Vec<String> = values
          .iter()
          .map(|value| self.push_param(params, value.clone()))
          .collect();
        Ok(format!(
          "{} IN ({})",
          self.dialect().quote_identifier(field),
          placeholders.join(", ")
        ))
      }
      Filter::NotIn(field, values) => {
        validate_identifier(field)?;
        if values.is_empty() {
          return Ok("1 = 1".to_string());
        }
        let placeholders: Vec<String> = values
          .iter()
          .map(|value| self.push_param(params, value.clone()))
          .collect();
        Ok(format!(
          "{} NOT IN ({})",
          self.dialect().quote_identifier(field),
          placeholders.join(", ")
        ))
      }
      Filter::ArrayContains(field, value) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, value.clone());
        Ok(format!(
          "JSON_CONTAINS({}, {})",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::Contains(field, value) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, Value::String(format!("%{}%", value)));
        Ok(format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::StartsWith(field, prefix) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, Value::String(format!("{}%", prefix)));
        Ok(format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::IsNull(field) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} IS NULL",
          self.dialect().quote_identifier(field)
        ))
      }
      Filter::IsNotNull(field) => {
        validate_identifier(field)?;
        Ok(format!(
          "{} IS NOT NULL",
          self.dialect().quote_identifier(field)
        ))
      }
      Filter::Like(field, pattern) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, Value::String(pattern.clone()));
        Ok(format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::EndsWith(field, suffix) => {
        validate_identifier(field)?;
        let placeholder = self.push_param(params, Value::String(format!("%{}", suffix)));
        Ok(format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          placeholder
        ))
      }
      Filter::Between(field, min, max) => {
        validate_identifier(field)?;
        let min_placeholder = self.push_param(params, min.clone());
        let max_placeholder = self.push_param(params, max.clone());
        Ok(format!(
          "{} BETWEEN {} AND {}",
          self.dialect().quote_identifier(field),
          min_placeholder,
          max_placeholder
        ))
      }
      Filter::And(filters) => {
        let mut parts = Vec::new();
        for filter in filters {
          parts.push(format!("({})", self.filter_to_bound_sql(filter, params)?));
        }
        Ok(parts.join(" AND "))
      }
      Filter::Or(filters) => {
        let mut parts = Vec::new();
        for filter in filters {
          parts.push(format!("({})", self.filter_to_bound_sql(filter, params)?));
        }
        Ok(parts.join(" OR "))
      }
      Filter::Not(inner) => Ok(format!(
        "NOT ({})",
        self.filter_to_bound_sql(inner, params)?
      )),
    }
  }
}
