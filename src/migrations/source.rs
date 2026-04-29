use std::path::PathBuf;

use crate::error::OrmResult;
use crate::migrations::migration::Migration;
use crate::provider::DatabaseProvider;
use std::marker::PhantomData;

pub enum MigrationSource {
  FileSystem { path: PathBuf },
  Embedded,
  SQL,
}

pub trait MigrationLoader<P: DatabaseProvider>: Send + Sync {
  fn load_migrations(&self) -> OrmResult<Vec<Box<dyn Migration<P>>>>;
}

pub struct SqlFileLoader<P: DatabaseProvider> {
  pub path: PathBuf,
  _phantom: PhantomData<P>,
}

impl<P: DatabaseProvider> SqlFileLoader<P> {
  pub fn new(path: impl Into<PathBuf>) -> Self {
    Self {
      path: path.into(),
      _phantom: PhantomData,
    }
  }
}

impl<P: DatabaseProvider> MigrationLoader<P> for SqlFileLoader<P> {
  fn load_migrations(&self) -> OrmResult<Vec<Box<dyn Migration<P>>>> {
    let mut migrations: Vec<Box<dyn Migration<P>>> = Vec::new();
    let entries = std::fs::read_dir(&self.path)?;

    for entry in entries.flatten() {
      let path = entry.path();
      if path.extension().and_then(|s| s.to_str()) == Some("sql") {
        let content = std::fs::read_to_string(&path)?;
        let stem = path
          .file_stem()
          .and_then(|s| s.to_str())
          .unwrap_or("unknown");

        let parts: Vec<&str> = stem.splitn(2, '_').collect();
        let version: i64 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
        let name = parts.get(1).unwrap_or(&stem).to_string();

        let sql_parts: Vec<&str> = content.split("--- DOWN ---").collect();
        let up_sql = sql_parts
          .first()
          .map(|s| s.trim())
          .unwrap_or("")
          .to_string();
        let down_sql = sql_parts.get(1).map(|s| s.trim()).unwrap_or("").to_string();

        let migration =
          crate::migrations::migration::SqlMigration::new(version, &name, &up_sql, &down_sql);
        migrations.push(Box::new(migration));
      }
    }

    migrations.sort_by_key(|a| a.version());
    Ok(migrations)
  }
}
