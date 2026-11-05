use crate::sql::types::SqlIndexDef;

use super::builder::SqlQueryBuilder;

impl SqlQueryBuilder {
  pub fn build_create_index(&self, index: &SqlIndexDef) -> String {
    index.to_sql(self.dialect())
  }
}
