use chrono::Utc;
use serde_json::json;

use crate::error::{OrmError, OrmResult};
use crate::provider::{AdminCommands, Storage};

use super::migration::{Migration, MigrationMeta};

const MIGRATIONS_COLLECTION: &str = "_migrations";

// The row that proves the collection has been created. It is bookkeeping,
// not a migration, so it is filtered out of `status()` and `rollback`.
const SCHEMA_MARKER_ID: &str = "_schema";

pub struct MigrationRunner<P: Storage> {
  provider: P,
  migrations: Vec<Box<dyn Migration<P>>>,
}

// `AdminCommands` is needed to create the migrations collection: document
// stores make one on first write, but a SQL provider needs its table to
// exist before anything can be inserted into it.
impl<P: Storage + AdminCommands> MigrationRunner<P> {
  pub fn new(provider: P) -> Self {
    Self {
      provider,
      migrations: Vec::new(),
    }
  }

  pub fn add_migration<M: Migration<P> + 'static>(&mut self, migration: M) {
    self.migrations.push(Box::new(migration));
  }

  // Applied records as `(id, meta)`. The raw id travels alongside the meta
  // because `rollback` has to delete exactly this row.
  //
  // A record's id *is* the serialised meta. That is not a stylistic choice:
  // `create_collection` guarantees an `id` column on every provider, but the
  // SQL providers shape it as `(id TEXT PRIMARY KEY)` only, so `version`,
  // `name` and `applied_at` have nowhere to live as their own columns.
  // Encoding them in the id makes the bookkeeping portable across every
  // backend without dialect-specific DDL.
  async fn get_applied_records(&self) -> OrmResult<Vec<(String, MigrationMeta)>> {
    let docs = self.provider.find_all(MIGRATIONS_COLLECTION).await?;
    let mut records: Vec<(String, MigrationMeta)> = Vec::new();

    for doc in docs {
      let Some(id) = doc.get("id").and_then(|v| v.as_str()).map(str::to_string) else {
        continue;
      };
      if id == SCHEMA_MARKER_ID {
        continue;
      }

      let meta = match serde_json::from_str::<MigrationMeta>(&id) {
        Ok(meta) => meta,
        // Records written before the id carried the meta kept the fields
        // beside a generated id; read those too rather than losing them.
        Err(_) => match serde_json::from_value::<MigrationMeta>(doc.clone()) {
          Ok(meta) => meta,
          Err(_) => continue,
        },
      };
      records.push((id, meta));
    }

    records.sort_by_key(|(_, meta)| meta.version);
    Ok(records)
  }

  async fn get_applied_migrations(&self) -> OrmResult<Vec<MigrationMeta>> {
    Ok(
      self
        .get_applied_records()
        .await?
        .into_iter()
        .map(|(_, meta)| meta)
        .collect(),
    )
  }

  async fn ensure_migrations_collection(&self) -> OrmResult<()> {
    // `exists` on a collection that is not there yet errors, which reads the
    // same as "no marker row" — so this is the cheap early out once the
    // marker has been written.
    if self
      .provider
      .exists(MIGRATIONS_COLLECTION, SCHEMA_MARKER_ID)
      .await
      .unwrap_or(false)
    {
      return Ok(());
    }

    // Create first: a SQL provider needs its table before an insert can
    // land, while document stores create the collection implicitly. A
    // failure here is usually just "already exists" — there is no
    // `IF NOT EXISTS` — so it is not fatal on its own: the marker insert
    // below is what validates the outcome, and it propagates a real error.
    let _ = self
      .provider
      .create_collection(MIGRATIONS_COLLECTION, None)
      .await;

    // Only `id`: that is the one column every provider's `create_collection`
    // is guaranteed to have made.
    self
      .provider
      .insert(MIGRATIONS_COLLECTION, json!({"id": SCHEMA_MARKER_ID}))
      .await?;
    Ok(())
  }

  pub async fn run_all_pending(&self) -> OrmResult<Vec<MigrationMeta>> {
    self.ensure_migrations_collection().await?;
    let applied = self.get_applied_migrations().await?;
    let applied_versions: Vec<i64> = applied.iter().map(|m| m.version).collect();
    let mut results: Vec<MigrationMeta> = Vec::new();

    for migration in &self.migrations {
      if !applied_versions.contains(&migration.version()) {
        // The body runs first: a migration that fails must not be recorded
        // as applied, or it could never be retried.
        migration.up(&self.provider).await?;
        let meta = MigrationMeta {
          version: migration.version(),
          name: migration.name().to_string(),
          applied_at: Some(Utc::now()),
        };
        let id = serde_json::to_string(&meta)?;
        self
          .provider
          .insert(MIGRATIONS_COLLECTION, json!({ "id": id }))
          .await?;
        results.push(meta);
      }
    }

    Ok(results)
  }

  pub async fn rollback(&self, count: u32) -> OrmResult<()> {
    let applied = self.get_applied_records().await?;
    let to_rollback: Vec<_> = applied.into_iter().rev().take(count as usize).collect();

    for (id, meta) in to_rollback {
      let migration = self
        .migrations
        .iter()
        .find(|m| m.version() == meta.version)
        .ok_or_else(|| {
          OrmError::InvalidInput(format!(
            "cannot roll back version {}: the migration is not registered",
            meta.version
          ))
        })?;

      migration.down(&self.provider).await?;

      // The record has to go too. Leaving it behind would keep the
      // migration marked as applied, so `run_all_pending` would never run
      // it again — a rollback that undid only half its own bookkeeping.
      self.provider.delete(MIGRATIONS_COLLECTION, &id).await?;
    }

    Ok(())
  }

  pub async fn status(&self) -> OrmResult<Vec<MigrationMeta>> {
    self.ensure_migrations_collection().await?;
    self.get_applied_migrations().await
  }
}
