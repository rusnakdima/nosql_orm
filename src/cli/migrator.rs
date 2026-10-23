use crate::error::OrmResult;
use crate::provider::Storage;

pub struct MigrationCommands;

impl MigrationCommands {
  pub async fn run<P: Storage>(provider: P) -> OrmResult<Vec<crate::MigrationMeta>> {
    let runner = crate::MigrationRunner::new(provider);
    runner.run_all_pending().await
  }

  pub async fn rollback<P: Storage>(provider: P, count: u32) -> OrmResult<()> {
    let runner = crate::MigrationRunner::new(provider);
    runner.rollback(count).await
  }

  pub async fn status<P: Storage>(provider: P) -> OrmResult<Vec<crate::MigrationMeta>> {
    let runner = crate::MigrationRunner::new(provider);
    runner.status().await
  }
}
