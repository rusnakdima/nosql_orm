use crate::cascade::CascadeManager;
use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::relations::WithRelations;
use crate::soft_delete::SoftDeletable;
use std::collections::HashSet;

use super::Repository;

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub async fn delete(&self, id: impl AsRef<str>) -> OrmResult<bool>
  where
    E: WithRelations,
  {
    let id_str = id.as_ref();

    let relations = E::relations();
    let has_cascade = relations.iter().any(|r| r.should_cascade_hard_delete());
    if has_cascade {
      let cascade = CascadeManager::new(self.provider.clone());
      let mut deleted = HashSet::new();
      return cascade
        .hard_delete_cascade::<E>(id_str, &relations, &mut deleted)
        .await;
    }

    if let Some(ref events) = self.events {
      if let Some(entity) = self.find_by_id(id_str).await? {
        events.dispatch_delete(&entity.to_value()?).await?;
      }
    }

    self.provider.delete(&Self::collection(), id_str).await
  }

  pub async fn remove(&self, entity: &E) -> OrmResult<bool>
  where
    E: WithRelations,
  {
    let id = entity
      .get_id()
      .ok_or_else(|| OrmError::InvalidQuery("Cannot remove entity without an id".to_string()))?;
    self.delete(&id).await
  }

  pub async fn soft_delete(&self, id: impl AsRef<str>) -> OrmResult<bool>
  where
    E: WithRelations + SoftDeletable,
  {
    let id_str = id.as_ref();

    let relations = E::relations();
    let has_cascade = relations.iter().any(|r| r.should_cascade_soft_delete());
    if has_cascade {
      let cascade = CascadeManager::new(self.provider.clone());
      let mut deleted = HashSet::new();
      return cascade
        .soft_delete_cascade::<E>(id_str, &relations, &mut deleted)
        .await;
    }

    let patch = serde_json::json!({ "deleted_at": chrono::Utc::now() });
    self
      .provider
      .patch(&Self::collection(), id_str, patch)
      .await?;
    Ok(true)
  }

  pub async fn restore(&self, id: impl AsRef<str>) -> OrmResult<bool> {
    let patch = serde_json::json!({ "deleted_at": serde_json::Value::Null });
    self
      .provider
      .patch(&Self::collection(), id.as_ref(), patch)
      .await?;
    Ok(true)
  }
}
