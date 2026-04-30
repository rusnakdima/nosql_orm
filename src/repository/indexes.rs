use crate::entity::Entity;
use crate::error::OrmResult;
use crate::provider::DatabaseProvider;

use super::Repository;

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub async fn create_index(&self, index: crate::nosql_index::NosqlIndex) -> OrmResult<()> {
    self
      .provider
      .create_index(&Self::collection(), &index)
      .await
  }

  pub async fn drop_index(&self, name: &str) -> OrmResult<()> {
    self.provider.drop_index(&Self::collection(), name).await
  }

  pub async fn list_indexes(&self) -> OrmResult<Vec<crate::nosql_index::NosqlIndexInfo>> {
    self.provider.list_indexes(&Self::collection()).await
  }

  pub async fn sync_indexes(&self) -> OrmResult<Vec<String>> {
    let manager = self.indexes();
    manager.sync_from_entity::<E>(&Self::collection()).await
  }
}
