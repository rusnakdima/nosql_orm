use crate::error::{OrmError, OrmResult};
use crate::query::Filter;

use super::builder::SqlQueryBuilder;

/// Check that `name` can be used as an identifier.
///
/// The compiler quotes every identifier it emits, and
/// [`SqlDialect::quote_identifier`](crate::sql::types::SqlDialect::quote_identifier)
/// escapes the dialect's quote character by doubling it. That escaping is
/// what makes a hostile name safe: it stays one identifier instead of
/// terminating the statement. So the only names rejected here are the ones
/// quoting cannot rescue — an empty name, one longer than the database's
/// identifier limit, and one carrying a NUL byte (which truncates the name
/// in C-based clients).
///
/// A name that merely *needs* quoting — one with a space, a dash, a leading
/// digit or SQL syntax in it — is not rejected. It is quoted and escaped.
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
  /// Render `filter` as a SQL `WHERE` clause body.
  ///
  /// Every field name is emitted as a quoted identifier, escaped by the
  /// dialect's quoting convention, so a field name carrying SQL syntax
  /// stays one identifier. Values are emitted as SQL literals.
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
}
