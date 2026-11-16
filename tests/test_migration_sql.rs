// SqlMigration body execution tests — gated on `sql-sqlite`.
//
// `SqlMigration::up`/`down` used to be no-ops that returned `Ok(())`, so the
// runner recorded a migration as applied without ever running it. These
// tests prove the statements really execute: the table appears on `up` and
// is gone after `down`.
//
// SQLite is used because it is bundled and needs no server, so the suite is
// hermetic. A provider with no query language answers `NotSupported`, which
// `test_sql_migration_rejects_a_provider_without_sql` pins separately.

use nosql_orm::error::OrmResult;
use nosql_orm::migrations::migration::{Migration, SqlMigration};
use nosql_orm::migrations::runner::MigrationRunner;
use nosql_orm::provider::{AdminCommands, Storage};
use nosql_orm::providers::sql::SqliteProvider;

async fn fresh() -> OrmResult<(SqliteProvider, tempfile::TempDir)> {
  let temp = tempfile::tempdir()?;
  let path = temp.path().join("migrations.db");
  let path = path
    .to_str()
    .ok_or_else(|| nosql_orm::error::OrmError::InvalidInput("bad path".into()))?
    .to_string();
  let provider = SqliteProvider::connect(path).await?;
  Ok((provider, temp))
}

async fn table_exists(provider: &SqliteProvider, name: &str) -> OrmResult<bool> {
  let result = provider
    .execute_raw(
      "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
      vec![serde_json::json!(name)],
    )
    .await?;
  Ok(!result.rows.is_empty())
}

#[tokio::test]
async fn sql_migration_up_actually_executes_the_statement() -> OrmResult<()> {
  let (provider, _temp) = fresh().await?;
  assert!(
    !table_exists(&provider, "users").await?,
    "the table must not exist before the migration runs"
  );

  let migration = SqlMigration::new(
    1,
    "create_users",
    "CREATE TABLE users (id TEXT PRIMARY KEY, name TEXT)",
    "DROP TABLE users",
  );
  migration.up(&provider).await?;

  assert!(
    table_exists(&provider, "users").await?,
    "up() must execute the CREATE TABLE"
  );
  Ok(())
}

#[tokio::test]
async fn sql_migration_down_actually_executes_the_statement() -> OrmResult<()> {
  let (provider, _temp) = fresh().await?;

  let migration = SqlMigration::new(
    1,
    "create_users",
    "CREATE TABLE users (id TEXT PRIMARY KEY)",
    "DROP TABLE users",
  );
  migration.up(&provider).await?;
  assert!(table_exists(&provider, "users").await?);

  migration.down(&provider).await?;
  assert!(
    !table_exists(&provider, "users").await?,
    "down() must execute the DROP TABLE"
  );
  Ok(())
}

// An empty statement is a legitimate one-directional migration: it must be
// skipped, not sent to the server to be rejected.
#[tokio::test]
async fn sql_migration_treats_an_empty_statement_as_a_no_op() -> OrmResult<()> {
  let (provider, _temp) = fresh().await?;

  let up_only = SqlMigration::new(1, "up_only", "CREATE TABLE a (id TEXT)", "");
  up_only.up(&provider).await?;
  assert!(table_exists(&provider, "a").await?);

  // down_sql is empty, so nothing happens — and nothing errors.
  up_only.down(&provider).await?;
  assert!(table_exists(&provider, "a").await?);

  // The reverse: a down-only migration.
  let down_only = SqlMigration::new(2, "down_only", "", "DROP TABLE a");
  down_only.up(&provider).await?;
  down_only.down(&provider).await?;
  assert!(!table_exists(&provider, "a").await?);
  Ok(())
}

// A failing statement must surface as an error, not be swallowed.
#[tokio::test]
async fn sql_migration_reports_a_failing_statement() -> OrmResult<()> {
  let (provider, _temp) = fresh().await?;

  let broken = SqlMigration::new(1, "broken", "NOT SQL AT ALL", "");
  let err = broken.up(&provider).await;
  assert!(err.is_err(), "a bad statement must not report success");
  Ok(())
}

// End to end: apply, roll back, and prove the bookkeeping followed — the
// record is deleted on rollback so a re-run applies the migration again.
#[tokio::test]
async fn runner_applies_records_rolls_back_and_reapplies() -> OrmResult<()> {
  let (provider, _temp) = fresh().await?;
  let mut runner = MigrationRunner::new(provider.clone());

  runner.add_migration(SqlMigration::new(
    1,
    "create_users",
    "CREATE TABLE users (id TEXT PRIMARY KEY)",
    "DROP TABLE users",
  ));

  let applied = runner.run_all_pending().await?;
  assert_eq!(applied.len(), 1, "the migration must be applied once");
  assert!(table_exists(&provider, "users").await?);

  // Already applied: a second run must change nothing.
  let again = runner.run_all_pending().await?;
  assert!(again.is_empty(), "an applied migration must be skipped");
  assert_eq!(
    runner.status().await?.len(),
    1,
    "status lists one migration"
  );

  // Roll back: the body runs AND the record is removed.
  runner.rollback(1).await?;
  assert!(
    !table_exists(&provider, "users").await?,
    "rollback must run the down statement"
  );
  assert_eq!(
    status_len(&runner).await?,
    0,
    "rollback must delete the applied record, or the migration could never re-run"
  );

  // …so the next run applies it again.
  let reapplied = runner.run_all_pending().await?;
  assert_eq!(reapplied.len(), 1, "a rolled-back migration must re-run");
  assert!(table_exists(&provider, "users").await?);
  Ok(())
}

async fn status_len(runner: &MigrationRunner<SqliteProvider>) -> OrmResult<usize> {
  Ok(runner.status().await?.len())
}

// Rolling back a version that is not registered must say so rather than
// silently leaving the record in place.
#[tokio::test]
async fn runner_refuses_to_roll_back_an_unregistered_migration() -> OrmResult<()> {
  let (provider, _temp) = fresh().await?;

  // Apply one migration, then build a *different* runner that has no
  // migrations registered at all.
  let mut first = MigrationRunner::new(provider.clone());
  first.add_migration(SqlMigration::new(
    1,
    "create_users",
    "CREATE TABLE users (id TEXT PRIMARY KEY)",
    "DROP TABLE users",
  ));
  first.run_all_pending().await?;

  let empty = MigrationRunner::new(provider.clone());
  let err = empty.rollback(1).await;
  assert!(
    matches!(&err, Err(nosql_orm::error::OrmError::InvalidInput(_))),
    "expected InvalidInput, got {:?}",
    err
  );
  Ok(())
}
