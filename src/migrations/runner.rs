use chrono::Utc;

use crate::error::OrmResult;
use crate::provider::DatabaseProvider;

use super::migration::{Migration, MigrationMeta};

const MIGRATIONS_COLLECTION: &str = "_migrations";

/// Runs migrations against a provider
pub struct MigrationRunner<P: DatabaseProvider> {
  provider: P,
  migrations: Vec<Box<dyn Migration<P>>>,
}

impl<P: DatabaseProvider> MigrationRunner<P> {
  pub fn new(provider: P) -> Self {
    Self {
      provider,
      migrations: Vec::new(),
    }
  }

  pub fn add_migration<M: Migration<P> + 'static>(&mut self, migration: M) {
    self.migrations.push(Box::new(migration));
  }

  async fn get_applied_migrations(&self) -> OrmResult<Vec<MigrationMeta>> {
    let docs = self.provider.find_all(MIGRATIONS_COLLECTION).await?;
    let mut metas: Vec<MigrationMeta> = Vec::new();
    for doc in docs {
      if let Ok(meta) = serde_json::from_value::<MigrationMeta>(doc) {
        metas.push(meta);
      }
    }
    metas.sort_by(|a, b| a.version.cmp(&b.version));
    Ok(metas)
  }

  async fn ensure_migrations_table(&self) -> OrmResult<()> {
    let exists = self
      .provider
      .exists(MIGRATIONS_COLLECTION, "_schema")
      .await
      .unwrap_or(false);
    if !exists {
      let schema = serde_json::json!({
          "_id": "_schema",
          "version": 0,
          "name": "schema",
          "applied_at": null
      });
      self.provider.insert(MIGRATIONS_COLLECTION, schema).await?;
    }
    Ok(())
  }

  pub async fn run_all_pending(&self) -> OrmResult<Vec<MigrationMeta>> {
    self.ensure_migrations_table().await?;
    let applied = self.get_applied_migrations().await?;
    let applied_versions: Vec<i64> = applied.iter().map(|m| m.version).collect();
    let mut results: Vec<MigrationMeta> = Vec::new();

    for migration in &self.migrations {
      if !applied_versions.contains(&migration.version()) {
        migration.up(&self.provider).await?;
        let meta = MigrationMeta {
          version: migration.version(),
          name: migration.name().to_string(),
          applied_at: Some(Utc::now()),
        };
        let doc = serde_json::to_value(&meta)?;
        self.provider.insert(MIGRATIONS_COLLECTION, doc).await?;
        results.push(meta);
      }
    }

    Ok(results)
  }

  pub async fn rollback(&self, count: u32) -> OrmResult<()> {
    let applied = self.get_applied_migrations().await?;
    let to_rollback: Vec<_> = applied.into_iter().rev().take(count as usize).collect();

    for meta in to_rollback {
      for migration in &self.migrations {
        if migration.version() == meta.version {
          migration.down(&self.provider).await?;
          break;
        }
      }
    }

    Ok(())
  }

  pub async fn status(&self) -> OrmResult<Vec<MigrationMeta>> {
    self.ensure_migrations_table().await?;
    self.get_applied_migrations().await
  }
}
