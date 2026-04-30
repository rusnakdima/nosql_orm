use crate::sql::types::{SqlIndexDef, SqlTableDef};

use super::builder::SqlQueryBuilder;

impl SqlQueryBuilder {
  pub fn create_table_sql(&self, table: &SqlTableDef) -> String {
    table.to_sql(self.dialect())
  }

  pub fn drop_table_sql(&self, table_name: &str) -> String {
    let name = self.dialect().quote_identifier(table_name);
    format!("DROP TABLE {}", name)
  }

  pub fn create_index_sql(&self, table_name: &str, index: &SqlIndexDef) -> String {
    let unique_str = if index.unique { "UNIQUE " } else { "" };
    let columns = index
      .columns
      .iter()
      .map(|c| self.dialect().quote_identifier(c))
      .collect::<Vec<_>>()
      .join(", ");
    let name = self.dialect().quote_identifier(&index.name);
    format!(
      "CREATE {}INDEX {} ON {} ({})",
      unique_str,
      name,
      self.dialect().quote_identifier(table_name),
      columns
    )
  }

  pub fn build_create_index(&self, index: &SqlIndexDef) -> String {
    index.to_sql(self.dialect())
  }

  pub fn build_drop_index(&self, table_name: &str, index_name: &str) -> String {
    let index = self.dialect().quote_identifier(index_name);
    let table = self.dialect().quote_identifier(table_name);
    format!("DROP INDEX {} ON {}", index, table)
  }

  pub fn build_create_table(&self, table: &SqlTableDef) -> String {
    table.to_sql(self.dialect())
  }

  pub fn build_drop_table(&self, table_name: &str, if_exists: bool) -> String {
    let name = self.dialect().quote_identifier(table_name);
    if if_exists {
      format!("DROP TABLE IF EXISTS {}", name)
    } else {
      format!("DROP TABLE {}", name)
    }
  }
}
