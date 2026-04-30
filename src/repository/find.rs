use crate::entity::{Entity, FrontendProjection};
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::query::Projection;
use serde_json::Value;

use super::Repository;

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub async fn find_by_id(&self, id: impl AsRef<str>) -> OrmResult<Option<E>> {
    match self
      .provider
      .find_by_id(&Self::collection(), id.as_ref())
      .await?
    {
      Some(v) => Ok(Some(E::from_value(v)?)),
      None => Ok(None),
    }
  }

  pub async fn get_by_id(&self, id: impl AsRef<str>) -> OrmResult<E> {
    self
      .find_by_id(id.as_ref())
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", Self::collection(), id.as_ref())))
  }

  pub async fn find_all(&self) -> OrmResult<Vec<E>> {
    if E::is_soft_deletable() {
      self.find_all_including_deleted().await
    } else {
      let docs = self.provider.find_all(&Self::collection()).await?;
      docs.into_iter().map(E::from_value).collect()
    }
  }

  pub async fn find_all_including_deleted(&self) -> OrmResult<Vec<E>> {
    let docs = self.provider.find_all(&Self::collection()).await?;
    docs.into_iter().map(E::from_value).collect()
  }

  pub async fn count(&self) -> OrmResult<u64> {
    self.provider.count(&Self::collection(), None).await
  }

  pub async fn exists(&self, id: impl AsRef<str>) -> OrmResult<bool> {
    self.provider.exists(&Self::collection(), id.as_ref()).await
  }

  pub async fn find_for_frontend(&self) -> OrmResult<Vec<E>>
  where
    E: FrontendProjection,
  {
    let projection = Projection::exclude(E::frontend_excluded_fields().as_slice());
    let docs = self.provider.find_all(&Self::collection()).await?;
    let filtered: Vec<Value> = docs
      .into_iter()
      .map(|doc| projection.apply_recursive(&doc))
      .collect();
    filtered.into_iter().map(E::from_value).collect()
  }

  pub async fn find_by_id_for_frontend(&self, id: impl AsRef<str>) -> OrmResult<Option<E>>
  where
    E: FrontendProjection,
  {
    let projection = Projection::exclude(E::frontend_excluded_fields().as_slice());
    match self
      .provider
      .find_by_id(&Self::collection(), id.as_ref())
      .await?
    {
      Some(v) => {
        let filtered = projection.apply_recursive(&v);
        Ok(Some(E::from_value(filtered)?))
      }
      None => Ok(None),
    }
  }
}
