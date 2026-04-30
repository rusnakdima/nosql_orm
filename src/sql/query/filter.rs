use crate::query::Filter;

use super::builder::SqlQueryBuilder;

impl SqlQueryBuilder {
  pub fn filter_to_sql(&self, filter: &Filter) -> String {
    match filter {
      Filter::Eq(field, value) => {
        format!(
          "{} = {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Ne(field, value) => {
        format!(
          "{} <> {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Gt(field, value) => {
        format!(
          "{} > {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Gte(field, value) => {
        format!(
          "{} >= {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Lt(field, value) => {
        format!(
          "{} < {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::Lte(field, value) => {
        format!(
          "{} <= {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(value)
        )
      }
      Filter::In(field, values) => {
        let values_str = values
          .iter()
          .map(|v| self.value_to_sql(v))
          .collect::<Vec<_>>()
          .join(", ");
        format!(
          "{} IN ({})",
          self.dialect().quote_identifier(field),
          values_str
        )
      }
      Filter::NotIn(field, values) => {
        let values_str = values
          .iter()
          .map(|v| self.value_to_sql(v))
          .collect::<Vec<_>>()
          .join(", ");
        format!(
          "{} NOT IN ({})",
          self.dialect().quote_identifier(field),
          values_str
        )
      }
      Filter::Contains(field, value) => {
        format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(&serde_json::json!(format!("%{}%", value)))
        )
      }
      Filter::StartsWith(field, prefix) => {
        format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(&serde_json::json!(format!("{}%", prefix)))
        )
      }
      Filter::IsNull(field) => {
        format!("{} IS NULL", self.dialect().quote_identifier(field))
      }
      Filter::IsNotNull(field) => {
        format!("{} IS NOT NULL", self.dialect().quote_identifier(field))
      }
      Filter::Like(field, pattern) => {
        format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(&serde_json::json!(pattern))
        )
      }
      Filter::EndsWith(field, suffix) => {
        format!(
          "{} LIKE {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(&serde_json::json!(format!("%{}", suffix)))
        )
      }
      Filter::Between(field, min, max) => {
        format!(
          "{} BETWEEN {} AND {}",
          self.dialect().quote_identifier(field),
          self.value_to_sql(min),
          self.value_to_sql(max)
        )
      }
      Filter::And(filters) => {
        let strs = filters
          .iter()
          .map(|f| format!("({})", self.filter_to_sql(f)))
          .collect::<Vec<_>>()
          .join(" AND ");
        strs
      }
      Filter::Or(filters) => {
        let strs = filters
          .iter()
          .map(|f| format!("({})", self.filter_to_sql(f)))
          .collect::<Vec<_>>()
          .join(" OR ");
        strs
      }
      Filter::Not(inner) => {
        format!("NOT ({})", self.filter_to_sql(inner))
      }
    }
  }
}
