use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexType {
  BTree,
  Hash,
  GiST,
  GIN,
  FullText,
}

#[derive(Debug, Clone)]
pub struct IndexDef {
  pub name: Option<String>,
  pub columns: Vec<String>,
  pub index_type: IndexType,
  pub unique: bool,
  pub concurrent: bool,
  pub where_clause: Option<String>,
}

impl IndexDef {
  pub fn new(columns: Vec<String>) -> Self {
    Self {
      name: None,
      columns,
      index_type: IndexType::BTree,
      unique: false,
      concurrent: false,
      where_clause: None,
    }
  }

  pub fn name(mut self, name: &str) -> Self {
    self.name = Some(name.to_string());
    self
  }

  pub fn unique(mut self) -> Self {
    self.unique = true;
    self
  }

  pub fn index_type(mut self, index_type: IndexType) -> Self {
    self.index_type = index_type;
    self
  }

  pub fn concurrent(mut self) -> Self {
    self.concurrent = true;
    self
  }

  pub fn where_clause(mut self, clause: &str) -> Self {
    self.where_clause = Some(clause.to_string());
    self
  }

  pub fn to_sql(&self, table: &str, provider: &str) -> String {
    let idx_name = self
      .name
      .clone()
      .unwrap_or_else(|| format!("idx_{}_{}", table, self.columns.join("_")));
    let unique = if self.unique { "UNIQUE " } else { "" };
    let index_type_sql = match (self.index_type, provider) {
      (IndexType::BTree, _) => "USING BTREE",
      (IndexType::Hash, "postgres") => "USING HASH",
      (IndexType::GiST, "postgres") => "USING GIST",
      (IndexType::GIN, "postgres") => "USING GIN",
      (IndexType::FullText, "postgres") => "USING GIN",
      (IndexType::FullText, _) => "",
      _ => "",
    };
    let columns = self.columns.join(", ");
    let where_clause = self
      .where_clause
      .as_ref()
      .map(|w| format!(" WHERE {}", w))
      .unwrap_or_default();

    format!(
      "CREATE {}INDEX {} {} ({}){}",
      unique, idx_name, index_type_sql, columns, where_clause
    )
  }
}

pub type Index = IndexDef;
