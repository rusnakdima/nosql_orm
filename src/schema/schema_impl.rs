use crate::constraints::{ColumnDef, IndexDef};
use crate::error::OrmResult;
use crate::schema::prefix::PrefixConfig;

#[derive(Debug, Clone)]
pub struct Schema {
  pub name: String,
  pub columns: Vec<ColumnDef>,
  pub indexes: Vec<IndexDef>,
  pub if_not_exists: bool,
}

impl Schema {
  pub fn new(name: &str) -> Self {
    Self {
      name: name.to_string(),
      columns: Vec::new(),
      indexes: Vec::new(),
      if_not_exists: true,
    }
  }

  pub fn add_column(mut self, column: ColumnDef) -> Self {
    self.columns.push(column);
    self
  }

  pub fn add_index(mut self, index: IndexDef) -> Self {
    self.indexes.push(index);
    self
  }

  pub fn if_not_exists(mut self) -> Self {
    self.if_not_exists = true;
    self
  }
}

#[derive(Debug, Clone, Default)]
pub struct SchemaManager {
  prefix_config: PrefixConfig,
}

impl SchemaManager {
  pub fn new() -> Self {
    Self {
      prefix_config: PrefixConfig::default(),
    }
  }

  pub fn with_prefix_config(mut self, config: PrefixConfig) -> Self {
    self.prefix_config = config;
    self
  }

  pub fn full_table_name(&self, collection: &str) -> String {
    self.prefix_config.apply(collection)
  }

  pub async fn create_collection(&self, schema: &Schema) -> OrmResult<()> {
    let table_name = self.full_table_name(&schema.name);
    let _ = table_name;
    Ok(())
  }

  pub async fn drop_collection(&self, name: &str) -> OrmResult<()> {
    let table_name = self.full_table_name(name);
    let _ = table_name;
    Ok(())
  }
}
