use crate::query::{Filter, OrderBy, Projection, SortDirection};

use super::builder::SqlQueryBuilder;

impl SqlQueryBuilder {
  pub fn insert_sql(&self, table: &str, data: &serde_json::Value) -> String {
    let table_name = self.dialect().quote_identifier(table);
    let obj = data.as_object().expect("data must be an object");
    let columns: Vec<String> = obj
      .keys()
      .map(|k| self.dialect().quote_identifier(k))
      .collect();
    let placeholders: Vec<String> = obj.keys().map(|_| "?".to_string()).collect();
    format!(
      "INSERT INTO {} ({}) VALUES ({})",
      table_name,
      columns.join(", "),
      placeholders.join(", ")
    )
  }

  pub fn update_sql(
    &self,
    table: &str,
    data: &serde_json::Value,
    pk_field: &str,
    _pk_value: &str,
  ) -> String {
    let table_name = self.dialect().quote_identifier(table);
    let obj = data.as_object().expect("data must be an object");
    let set_clause = obj
      .keys()
      .map(|k| format!("{} = ?", self.dialect().quote_identifier(k)))
      .collect::<Vec<_>>()
      .join(", ");
    format!(
      "UPDATE {} SET {} WHERE {} = ?",
      table_name,
      set_clause,
      self.dialect().quote_identifier(pk_field)
    )
  }

  pub fn delete_sql(&self, table: &str, pk_field: &str, _pk_value: &str) -> String {
    let table_name = self.dialect().quote_identifier(table);
    format!(
      "DELETE FROM {} WHERE {} = ?",
      table_name,
      self.dialect().quote_identifier(pk_field)
    )
  }

  pub fn select_sql(
    &self,
    table: &str,
    projection: Option<&[String]>,
    limit: Option<u32>,
    offset: Option<u64>,
  ) -> String {
    let table_name = self.dialect().quote_identifier(table);
    let select_clause = projection
      .map(|p| {
        if p.is_empty() {
          "*".to_string()
        } else {
          p.iter()
            .map(|f| self.dialect().quote_identifier(f))
            .collect::<Vec<_>>()
            .join(", ")
        }
      })
      .unwrap_or_else(|| "*".to_string());

    let mut sql = format!("SELECT {} FROM {}", select_clause, table_name);
    sql.push_str(" ORDER BY id ASC");
    if let Some(l) = limit {
      sql.push_str(&format!(" LIMIT {}", l));
    }
    if let Some(o) = offset {
      sql.push_str(&format!(" OFFSET {}", o));
    }
    sql
  }

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

  pub fn build_select(
    &self,
    table: &str,
    filter: Option<&Filter>,
    projection: Option<&Projection>,
    order_by: Option<&[OrderBy]>,
    limit: Option<u32>,
    offset: Option<u64>,
  ) -> String {
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
          let all_cols = self.get_table_columns(table);
          all_cols
            .iter()
            .filter(|c| !fields.contains(c))
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
      sql.push_str(&format!(" WHERE {}", self.filter_to_sql(f)));
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

    sql
  }

  pub fn build_update(
    &self,
    table: &str,
    set_columns: &[(&str, String)],
    filter: Option<&Filter>,
  ) -> String {
    let table_name = self.dialect().quote_identifier(table);

    let set_clause = set_columns
      .iter()
      .map(|(col, _)| format!("{} = ?", self.dialect().quote_identifier(col)))
      .collect::<Vec<_>>()
      .join(", ");

    let mut sql = format!("UPDATE {} SET {}", table_name, set_clause);

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.filter_to_sql(f)));
    }

    sql
  }

  pub fn build_delete(&self, table: &str, filter: Option<&Filter>) -> String {
    let table_name = self.dialect().quote_identifier(table);

    let mut sql = format!("DELETE FROM {}", table_name);

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.filter_to_sql(f)));
    }

    sql
  }

  pub fn build_count(&self, table: &str, filter: Option<&Filter>) -> String {
    let table_name = self.dialect().quote_identifier(table);

    let mut sql = format!("SELECT COUNT(*) FROM {}", table_name);

    if let Some(f) = filter {
      sql.push_str(&format!(" WHERE {}", self.filter_to_sql(f)));
    }

    sql
  }
}
