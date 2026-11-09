use crate::error::{OrmError, OrmResult};
use crate::query::{Filter, OrderBy, Projection, SortDirection};

use super::builder::SqlQueryBuilder;

impl SqlQueryBuilder {
  pub fn build_insert(&self, table: &str, columns: &[&str], values_count: usize) -> String {
    let table_name = self.dialect().quote_identifier(table);
    let cols = columns
      .iter()
      .map(|c| self.dialect().quote_identifier(c))
      .collect::<Vec<_>>()
      .join(", ");

    let placeholders = (0..values_count)
      .map(|i| self.dialect().parameter_placeholder(i))
      .collect::<Vec<_>>()
      .join(", ");

    format!(
      "INSERT INTO {} ({}) VALUES ({})",
      table_name, cols, placeholders
    )
  }

  // Eight parameters is the shape of a SELECT (table, filter, projection,
  // order, paging, columns) — a struct split would only move the fields.
  #[allow(clippy::too_many_arguments)]
  pub fn build_select(
    &self,
    table: &str,
    filter: Option<&Filter>,
    projection: Option<&Projection>,
    order_by: Option<&[OrderBy]>,
    limit: Option<u32>,
    offset: Option<u64>,
    all_columns: Option<&[String]>,
  ) -> OrmResult<String> {
    let table_name = self.dialect().quote_identifier(table);

    let select_clause = match projection {
      Some(p) => {
        if let Some(ref fields) = p.select {
          if fields.is_empty() {
            "*".to_string()
          } else {
            fields
              .iter()
              .map(|f| self.dialect().quote_identifier(f))
              .collect::<Vec<_>>()
              .join(", ")
          }
        } else if let Some(ref fields) = p.exclude {
          // An exclude projection names the fields to drop, not the ones to
          // keep: the remaining columns are the table's own columns minus
          // those, and only the database knows them. Without that column
          // list the statement cannot be built — and falling back to the
          // whole table would hand back the very fields the caller asked
          // to drop.
          let all_cols = all_columns.ok_or_else(|| {
            OrmError::InvalidQuery(format!(
              "an exclude projection on '{}' needs the table's columns",
              table
            ))
          })?;
          let remaining: Vec<&String> = all_cols.iter().filter(|c| !fields.contains(c)).collect();
          if remaining.is_empty() {
            return Err(OrmError::InvalidQuery(format!(
              "the exclude projection on '{}' leaves no column to select",
              table
            )));
          }
          remaining
            .iter()
            .map(|c| self.dialect().quote_identifier(c))
            .collect::<Vec<_>>()
            .join(", ")
        } else {
          "*".to_string()
        }
      }
      None => "*".to_string(),
    };

    let mut sql = format!("SELECT {} FROM {}", select_clause, table_name);

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.filter_to_sql(f)?));
    }

    if let Some(order) = order_by {
      if !order.is_empty() {
        let order_str = order
          .iter()
          .map(|o| {
            let dir = match o.direction {
              SortDirection::Asc => "ASC",
              SortDirection::Desc => "DESC",
            };
            format!("{} {}", self.dialect().quote_identifier(&o.field), dir)
          })
          .collect::<Vec<_>>()
          .join(", ");
        sql.push_str(&format!(" ORDER BY {}", order_str));
      }
    }

    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }

    if let Some(o) = offset {
      sql.push_str(&format!(" OFFSET {}", o));
    }

    Ok(sql)
  }
}
