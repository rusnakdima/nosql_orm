use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use crate::timestamps::apply_timestamps;
use crate::utils::generate_id;
use crate::validators::Validate;
use serde_json::Value;

use super::Repository;

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub async fn insert(&self, mut entity: E) -> OrmResult<E>
  where
    E: Validate,
  {
    entity.validate()?;
    if entity.get_id().is_none() {
      entity.set_id(generate_id());
    }
    let mut doc = entity.to_value()?;
    apply_timestamps(&mut doc, true);
    let stored = self.provider.insert(&Self::collection(), doc).await?;
    let result = E::from_value(stored)?;
    if let Some(ref events) = self.events {
      events.dispatch_insert(&result.to_value()?).await?;
    }
    Ok(result)
  }

  pub async fn update(&self, entity: E) -> OrmResult<E>
  where
    E: Validate,
  {
    entity.validate()?;
    let id = entity
      .get_id()
      .ok_or_else(|| OrmError::InvalidQuery("Cannot update entity without an id".to_string()))?;
    let before_doc = self.provider.find_by_id(&Self::collection(), &id).await?;
    let mut doc = entity.to_value()?;
    apply_timestamps(&mut doc, false);
    let stored = self.provider.update(&Self::collection(), &id, doc).await?;
    let result = E::from_value(stored)?;
    if let Some(ref events) = self.events {
      let before_value = before_doc.clone().unwrap_or_else(|| serde_json::json!({}));
      events
        .dispatch_update(&before_value, &result.to_value()?)
        .await?;
    }
    Ok(result)
  }

  pub async fn save(&self, entity: E) -> OrmResult<E>
  where
    E: Validate,
  {
    if entity.get_id().is_some() {
      self.update(entity).await
    } else {
      self.insert(entity).await
    }
  }

  pub async fn insert_many(&self, entities: Vec<E>) -> OrmResult<usize> {
    if entities.is_empty() {
      return Ok(0);
    }
    let mut count = 0;
    for mut entity in entities {
      if entity.get_id().is_none() {
        entity.set_id(generate_id());
      }
      let mut doc = entity.to_value()?;
      apply_timestamps(&mut doc, true);
      self.provider.insert(&Self::collection(), doc).await?;
      count += 1;
    }
    Ok(count)
  }

  pub async fn update_many(&self, filter: Option<Filter>, updates: Value) -> OrmResult<usize> {
    self
      .provider
      .update_many(&Self::collection(), filter, updates)
      .await
  }

  pub async fn upsert_many(&self, entities: Vec<E>) -> OrmResult<usize>
  where
    E: Validate,
  {
    if entities.is_empty() {
      return Ok(0);
    }
    let mut count = 0;
    for entity in entities {
      self.save(entity).await?;
      count += 1;
    }
    Ok(count)
  }

  pub async fn delete_many(&self, filter: Option<Filter>) -> OrmResult<usize> {
    self.provider.delete_many(&Self::collection(), filter).await
  }

  pub async fn patch(&self, id: impl AsRef<str>, patch: Value) -> OrmResult<E> {
    let stored = self
      .provider
      .patch(&Self::collection(), id.as_ref(), patch)
      .await?;
    E::from_value(stored)
  }
}
