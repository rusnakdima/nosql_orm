//! Shared SQL statement construction for the three SQL providers.
//!
//! SQLite, MySQL and PostgreSQL speak the same statements; they differ in
//! how a statement is *spelled*: SQLite and MySQL bind a parameter with
//! `?`, PostgreSQL with `$1`, MySQL drops an index through the table that
//! owns it and renames a table with `RENAME TABLE`. That difference lives in
//! [`SqlDialect`] and in the dispatch inside this module, so a provider says
//! *what* it wants and never *how* it is spelled.

use crate::nosql_index::NosqlIndex;
use crate::sql::types::SqlDialect;
use crate::sql::SqlQueryBuilder;
use serde_json::Value;

/// The statements every SQL provider builds the same way.
pub trait SqlProviderHelpers {
  /// The dialect this provider speaks.
  fn dialect(&self) -> SqlDialect;

  /// The query builder this provider renders statements with.
  fn query_builder(&self) -> &SqlQueryBuilder;

  /// Quote `name` as an identifier for this provider's dialect.
  fn quote_identifier(&self, name: &str) -> String {
    self.dialect().quote_identifier(name)
  }

  /// The placeholder that binds the `index`-th parameter.
  fn placeholder(&self, index: usize) -> String {
    self.dialect().parameter_placeholder(index)
  }

  /// Render `value` as an SQL literal.
  fn value_to_sql(&self, value: &Value) -> String {
    self.query_builder().value_to_sql(value)
  }

  /// The clause a write statement ends with when the dialect hands the
  /// written row back. Empty for the dialects that do not.
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
    format!(
      "DROP TABLE IF EXISTS {}",
      self.quote_identifier(collection)
    )
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

  /// The `INSERT` for `collection`, with the column names quoted.
  fn build_insert_sql(
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
      "INSERT INTO {} ({}) VALUES ({}){}",
      self.quote_identifier(collection),
      columns_str,
      values.join(", "),
      self.returning_clause()
    )
  }

  /// The `SELECT` that reads one document by its id.
  fn build_select_by_id_sql(&self, collection: &str) -> String {
    format!(
      "SELECT * FROM {} WHERE id = {}",
      self.quote_identifier(collection),
      self.placeholder(0)
    )
  }

  /// The `SELECT` that reads every document of a collection.
  fn build_select_all_sql(&self, collection: &str) -> String {
    format!("SELECT * FROM {}", self.quote_identifier(collection))
  }

  /// The `SELECT` that counts the documents of a collection.
  fn build_count_sql(&self, collection: &str) -> String {
    format!("SELECT COUNT(*) FROM {}", self.quote_identifier(collection))
  }

  /// The `UPDATE` that writes one document by its id.
  fn build_update_by_id_sql(&self, collection: &str, set_clauses: &[String]) -> String {
    format!(
      "UPDATE {} SET {} WHERE id = {}{}",
      self.quote_identifier(collection),
      set_clauses.join(", "),
      self.placeholder(0),
      self.returning_clause()
    )
  }

  /// The `UPDATE` that writes every document a filter matches.
  fn build_update_sql(&self, collection: &str, set_clauses: &[String]) -> String {
    format!(
      "UPDATE {} SET {}",
      self.quote_identifier(collection),
      set_clauses.join(", ")
    )
  }

  /// The `DELETE` that removes one document by its id.
  fn build_delete_by_id_sql(&self, collection: &str) -> String {
    format!(
      "DELETE FROM {} WHERE id = {}",
      self.quote_identifier(collection),
      self.placeholder(0)
    )
  }

  /// The `DELETE` that removes every document a filter matches.
  fn build_delete_sql(&self, collection: &str) -> String {
    format!("DELETE FROM {}", self.quote_identifier(collection))
  }

  /// The `CREATE INDEX` for `index` on `collection`.
  ///
  /// The index definition is built here, once: the name falls back to
  /// `idx_default`, the fields are the index's own, and a unique index says
  /// so. The dialect decides how the statement is spelled.
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

  /// The `DROP INDEX` for `index_name`.
  ///
  /// MySQL drops an index through the table that owns it; the other two
  /// dialects drop it by name.
  fn build_drop_index_sql(&self, collection: &str, index_name: &str) -> String {
    match self.dialect() {
      SqlDialect::MySQL => format!(
        "DROP INDEX {} ON {}",
        self.quote_identifier(index_name),
        self.quote_identifier(collection)
      ),
      SqlDialect::SQLite | SqlDialect::PostgreSQL => format!(
        "DROP INDEX IF EXISTS {}",
        self.quote_identifier(index_name)
      ),
    }
  }
}
