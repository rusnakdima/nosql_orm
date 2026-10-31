use crate::sql::types::SqlIndexDef;

use super::builder::SqlQueryBuilder;

impl SqlQueryBuilder {
  /// Render the `CREATE INDEX` statement for `index`.
  ///
  /// The three SQL providers call this to create an index, so it is the one
  /// builder in this module that is live.
  pub fn build_create_index(&self, index: &SqlIndexDef) -> String {
    index.to_sql(self.dialect())
  }
}
