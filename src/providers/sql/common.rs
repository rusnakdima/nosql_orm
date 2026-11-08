use crate::nosql_index::NosqlIndex;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use serde_json::Value;

pub trait SqlProviderHelpers {
  fn dialect(&self) -> SqlDialect;

  fn query_builder(&self) -> &SqlQueryBuilder;

  fn quote_identifier(&self, name: &str) -> String {
    self.dialect().quote_identifier(name)
  }

  fn placeholder(&self, index: usize) -> String {
    self.dialect().parameter_placeholder(index)
  }

  fn value_to_sql(&self, value: &Value) -> String {
    self.query_builder().value_to_sql(value)
  }

  fn returning_clause(&self) -> &'static str {
    match self.dialect() {
      SqlDialect::PostgreSQL => " RETURNING *",
      SqlDialect::SQLite | SqlDialect::MySQL => "",
    }
  }

  fn build_create_table_sql(&self, collection: &str) -> String {
    format!(
      "CREATE TABLE {} (id TEXT PRIMARY KEY)",
      self.quote_identifier(collection)
    )
  }

  fn build_drop_table_sql(&self, collection: &str) -> String {
    format!("DROP TABLE IF EXISTS {}", self.quote_identifier(collection))
  }

  fn build_rename_table_sql(&self, from: &str, to: &str) -> String {
    match self.dialect() {
      SqlDialect::MySQL => format!(
        "RENAME TABLE {} TO {}",
        self.quote_identifier(from),
        self.quote_identifier(to)
      ),
      SqlDialect::SQLite | SqlDialect::PostgreSQL => format!(
        "ALTER TABLE {} RENAME TO {}",
        self.quote_identifier(from),
        self.quote_identifier(to)
      ),
    }
  }

  fn build_insert_sql(&self, collection: &str, columns: &[&str], values: &[String]) -> String {
    let columns_str = columns
      .iter()
      .map(|c| self.quote_identifier(c))
      .collect::<Vec<_>>()
      .join(", ");

    format!(
      "INSERT INTO {} ({}) VALUES ({}){}",
      self.quote_identifier(collection),
      columns_str,
      values.join(", "),
      self.returning_clause()
    )
  }

  // INSERT without `RETURNING` — the shared `Storage` implementation
  // (`providers/sql/executor.rs`) reads the document back itself, so every
  // backend runs the same statement shape.
  fn build_insert_plain_sql(
    &self,
    collection: &str,
    columns: &[&str],
    values: &[String],
  ) -> String {
    let columns_str = columns
      .iter()
      .map(|c| self.quote_identifier(c))
      .collect::<Vec<_>>()
      .join(", ");

    format!(
      "INSERT INTO {} ({}) VALUES ({})",
      self.quote_identifier(collection),
      columns_str,
      values.join(", ")
    )
  }

  // `UPDATE … WHERE id = …` without `RETURNING`; the caller reads the row
  // back the same way on every backend.
  fn build_update_by_id_plain_sql(&self, collection: &str, set_clauses: &[String]) -> String {
    format!(
      "UPDATE {} SET {} WHERE id = {}",
      self.quote_identifier(collection),
      set_clauses.join(", "),
      self.placeholder(0)
    )
  }

  fn build_select_by_id_sql(&self, collection: &str) -> String {
    format!(
      "SELECT * FROM {} WHERE id = {}",
      self.quote_identifier(collection),
      self.placeholder(0)
    )
  }

  fn build_select_all_sql(&self, collection: &str) -> String {
    format!("SELECT * FROM {}", self.quote_identifier(collection))
  }

  fn build_count_sql(&self, collection: &str) -> String {
    format!("SELECT COUNT(*) FROM {}", self.quote_identifier(collection))
  }

  fn build_update_by_id_sql(&self, collection: &str, set_clauses: &[String]) -> String {
    format!(
      "UPDATE {} SET {} WHERE id = {}{}",
      self.quote_identifier(collection),
      set_clauses.join(", "),
      self.placeholder(0),
      self.returning_clause()
    )
  }

  fn build_update_sql(&self, collection: &str, set_clauses: &[String]) -> String {
    format!(
      "UPDATE {} SET {}",
      self.quote_identifier(collection),
      set_clauses.join(", ")
    )
  }

  fn build_delete_by_id_sql(&self, collection: &str) -> String {
    format!(
      "DELETE FROM {} WHERE id = {}",
      self.quote_identifier(collection),
      self.placeholder(0)
    )
  }

  fn build_delete_sql(&self, collection: &str) -> String {
    format!("DELETE FROM {}", self.quote_identifier(collection))
  }

  fn build_create_index_sql(&self, collection: &str, index: &NosqlIndex) -> String {
    let mut index_def = crate::sql::types::SqlIndexDef::new(
      index.get_name().unwrap_or("idx_default"),
      collection,
      index.get_fields().iter().map(|(f, _)| f.clone()).collect(),
    );

    if index.is_unique() {
      index_def = index_def.unique();
    }

    self.query_builder().build_create_index(&index_def)
  }

  fn build_drop_index_sql(&self, collection: &str, index_name: &str) -> String {
    match self.dialect() {
      SqlDialect::MySQL => format!(
        "DROP INDEX {} ON {}",
        self.quote_identifier(index_name),
        self.quote_identifier(collection)
      ),
      SqlDialect::SQLite | SqlDialect::PostgreSQL => {
        format!("DROP INDEX IF EXISTS {}", self.quote_identifier(index_name))
      }
    }
  }
}
