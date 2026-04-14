use crate::error::OrmResult;
use crate::provider::DatabaseProvider;

pub struct MigrationCommands;

impl MigrationCommands {
  pub async fn run<P: DatabaseProvider>(provider: P) -> OrmResult<Vec<crate::MigrationMeta>> {
    let runner = crate::MigrationRunner::new(provider);
    runner.run_all_pending().await
  }

  pub async fn rollback<P: DatabaseProvider>(provider: P, count: u32) -> OrmResult<()> {
    let runner = crate::MigrationRunner::new(provider);
    runner.rollback(count).await
  }

  pub async fn status<P: DatabaseProvider>(provider: P) -> OrmResult<Vec<crate::MigrationMeta>> {
    let runner = crate::MigrationRunner::new(provider);
    runner.status().await
  }
}
