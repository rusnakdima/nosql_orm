use crate::constraints::{ColumnDef, IndexDef};
use crate::error::{OrmError, OrmResult};

#[derive(Debug, Clone)]
pub enum SchemaChangeType {
    AddColumn { column: ColumnDef },
    RemoveColumn { column_name: String },
    ModifyColumn { old: ColumnDef, new: ColumnDef },
    RenameTable { old_name: String, new_name: String },
    AddIndex { index: IndexDef },
    RemoveIndex { index_name: String },
}

pub struct SchemaEvolution {
    changes: Vec<SchemaChangeType>,
}

impl SchemaEvolution {
    pub fn new() -> Self {
        Self { changes: Vec::new() }
    }

    pub fn add_column(mut self, column: ColumnDef) -> Self {
        self.changes.push(SchemaChangeType::AddColumn { column });
        self
    }

    pub fn remove_column(mut self, name: &str) -> Self {
        self.changes.push(SchemaChangeType::RemoveColumn {
            column_name: name.to_string(),
        });
        self
    }

    pub fn modify_column(mut self, old: ColumnDef, new: ColumnDef) -> Self {
        self.changes
            .push(SchemaChangeType::ModifyColumn { old, new });
        self
    }

    pub fn rename_table(mut self, old_name: &str, new_name: &str) -> Self {
        self.changes
            .push(SchemaChangeType::RenameTable {
                old_name: old_name.to_string(),
                new_name: new_name.to_string(),
            });
        self
    }

    pub fn add_index(mut self, index: IndexDef) -> Self {
        self.changes.push(SchemaChangeType::AddIndex { index });
        self
    }

    pub fn remove_index(mut self, index_name: &str) -> Self {
        self.changes
            .push(SchemaChangeType::RemoveIndex {
                index_name: index_name.to_string(),
            });
        self
    }

    pub fn build_migration(&self, table_name: &str) -> Vec<String> {
        let mut migrations = Vec::new();

        for change in &self.changes {
            match change {
                SchemaChangeType::AddColumn { column } => {
                    migrations.push(format!(
                        "ALTER TABLE {} ADD COLUMN {} {}",
                        table_name,
                        column.name,
                        column.column_type.as_sql("generic")
                    ));
                }
                SchemaChangeType::RemoveColumn { column_name } => {
                    migrations.push(format!(
                        "ALTER TABLE {} DROP COLUMN {}",
                        table_name, column_name
                    ));
                }
                SchemaChangeType::ModifyColumn { old: _, new } => {
                    migrations.push(format!(
                        "ALTER TABLE {} MODIFY COLUMN {} {}",
                        table_name,
                        new.name,
                        new.column_type.as_sql("generic")
                    ));
                }
                SchemaChangeType::RenameTable { old_name, new_name } => {
                    migrations.push(format!("ALTER TABLE {} RENAME TO {}", old_name, new_name));
                }
                SchemaChangeType::AddIndex { index } => {
                    migrations.push(format!(
                        "CREATE {} INDEX {} ON {} ({})",
                        if index.unique { "UNIQUE" } else { "" },
                        index.name,
                        table_name,
                        index.columns.join(", ")
                    ));
                }
                SchemaChangeType::RemoveIndex { index_name } => {
                    migrations.push(format!("DROP INDEX {} ON {}", index_name, table_name));
                }
            }
        }

        migrations
    }

    pub fn validate(&self) -> OrmResult<()> {
        for change in &self.changes {
            match change {
                SchemaChangeType::ModifyColumn { old, new } => {
                    if old.name != new.name {
                        return Err(OrmError::InvalidInput(
                            "Cannot rename column with ModifyColumn, use RenameTable instead".to_string(),
                        ));
                    }
                }
                SchemaChangeType::RenameTable { old_name, new_name } => {
                    if old_name.is_empty() || new_name.is_empty() {
                        return Err(OrmError::InvalidInput(
                            "Table names cannot be empty".to_string(),
                        ));
                    }
                }
                _ => return Err(OrmError::InvalidInput(
                    format!("Unhandled case in {}", std::any::type_name::<Self>())
                )),
            }
        }
        Ok(())
    }
}

impl Default for SchemaEvolution {
    fn default() -> Self {
        Self::new()
    }
}