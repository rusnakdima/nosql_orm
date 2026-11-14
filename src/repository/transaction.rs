use crate::entity::Entity;
use crate::error::OrmResult;
use crate::provider::Storage;
use super::Repository;

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: Storage + crate::provider::TransactionControl,
{
  pub async fn begin_transaction(&self) -> OrmResult<crate::provider::TransactionId> {
    self.provider.begin_transaction().await
  }

  pub async fn commit_transaction(
    &self,
    id: crate::provider::TransactionId,
  ) -> OrmResult<()> {
    self.provider.commit_transaction(id).await
  }

  pub async fn rollback_transaction(
    &self,
    id: crate::provider::TransactionId,
  ) -> OrmResult<()> {
    self.provider.rollback_transaction(id).await
  }

  pub async fn is_transaction_active(&self) -> OrmResult<bool> {
    self.provider.is_transaction_active().await
  }
}
