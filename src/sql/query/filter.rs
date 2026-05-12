use crate::error::{OrmError, OrmResult};
use crate::query::Filter;

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
  for (i, c) in name.chars().enumerate() {
    if i == 0 && c.is_ascii_digit() {
      return Err(OrmError::InvalidInput(format!(
        "Identifier '{}' cannot start with a digit",
        name
      )));
    }
    if !c.is_ascii_alphanumeric() && c != '_' {
      return Err(OrmError::InvalidInput(format!(
        "Identifier '{}' contains invalid character '{}'",
        name, c
      )));
    }
  }
  let sql_keywords = [
    "SELECT", "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "EXEC", "EXECUTE", "UNION",
    "WHERE", "FROM", "TABLE", "INDEX", "DATABASE", "SCHEMA", "GRANT", "REVOKE",
  ];
  let upper = name.to_uppercase();
  for keyword in sql_keywords {
    if upper == keyword {
      return Err(OrmError::InvalidInput(format!(
        "Identifier '{}' is a reserved keyword",
        name
      )));
    }
  }
  Ok(())
}

impl SqlQueryBuilder {
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
}
