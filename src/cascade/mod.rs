use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::relations::{RelationDef, WithRelations};

pub mod hard_delete;
pub mod helpers;
pub mod restore;
pub mod soft_delete;

pub struct CascadeManager<P: DatabaseProvider> {
  provider: P,
}

impl<P: DatabaseProvider> CascadeManager<P> {
  pub fn new(provider: P) -> Self {
    Self { provider }
  }

  pub async fn check_restrict<E: Entity + WithRelations>(
    &self,
    entity_id: &str,
    relations: &[RelationDef],
  ) -> OrmResult<bool> {
    for rel in relations {
      if self.should_restrict_on_delete(rel) {
        let has_related = self.has_related_entities::<E>(entity_id, rel).await?;
        if has_related {
          return Err(OrmError::CascadeRestricted {
            entity: E::table_name(),
            relation: rel.name.clone(),
          });
        }
      }
    }
    Ok(true)
  }

  async fn has_related_entities<E: Entity>(
    &self,
    entity_id: &str,
    relation: &RelationDef,
  ) -> OrmResult<bool> {
    use crate::cascade::helpers::cascade_value;
    use crate::query::Filter;
    use crate::relations::RelationType;

    match relation.relation_type {
      RelationType::OneToMany => {
        let filter = Filter::Eq(relation.foreign_key.clone(), cascade_value(entity_id));
        let count = self
          .provider
          .count(&relation.target_collection, Some(&filter))
          .await?;
        Ok(count > 0)
      }
      RelationType::ManyToOne => {
        let parent = self
          .provider
          .find_by_id(&E::table_name(), entity_id)
          .await?;
        if let Some(p) = parent {
          if let Some(foreign_id) = p.get(&relation.local_key).and_then(|v| v.as_str()) {
            let exists = self
              .provider
              .exists(&relation.target_collection, foreign_id)
              .await?;
            return Ok(exists);
          }
        }
        Ok(false)
      }
      RelationType::OneToOne => {
        let parent = self
          .provider
          .find_by_id(&E::table_name(), entity_id)
          .await?;
        if let Some(p) = parent {
          if let Some(foreign_id) = p.get(&relation.local_key).and_then(|v| v.as_str()) {
            let exists = self
              .provider
              .exists(&relation.target_collection, foreign_id)
              .await?;
            return Ok(exists);
          }
        }
        Ok(false)
      }
      RelationType::ManyToMany => {
        let entity = self
          .provider
          .find_by_id(&E::table_name(), entity_id)
          .await?;
        if let Some(e) = entity {
          let join_field = relation.join_field.as_deref().unwrap_or("ids");
          if let Some(arr) = e.get(join_field).and_then(|v| v.as_array()) {
            return Ok(!arr.is_empty());
          }
        }
        Ok(false)
      }
    }
  }

  fn should_cascade_soft_delete(&self, relation: &RelationDef) -> bool {
    relation.should_cascade_soft_delete()
  }

  fn should_cascade_hard_delete(&self, relation: &RelationDef) -> bool {
    relation.should_cascade_hard_delete()
  }

  fn should_restrict_on_delete(&self, relation: &RelationDef) -> bool {
    relation.should_restrict()
  }

  pub async fn soft_delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let patch = serde_json::json!({ "deleted_at": chrono::Utc::now() });
    self.provider.patch(collection, id, patch).await?;
    Ok(true)
  }

  pub async fn restore(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let patch = serde_json::json!({
      "deleted_at": serde_json::Value::Null,
      "restored_at": crate::timestamps::timestamp_now_rfc3339()
    });
    self.provider.patch(collection, id, patch).await?;
    Ok(true)
  }

  pub async fn toggle_delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let doc = self.provider.find_by_id(collection, id).await?;
    match doc {
      Some(d) => {
        let is_deleted = d.get("deleted_at").map(|v| !v.is_null()).unwrap_or(false);
        if is_deleted {
          self.restore(collection, id).await
        } else {
          self.soft_delete(collection, id).await
        }
      }
      None => Err(crate::error::OrmError::NotFound(format!(
        "Entity {} not found in {}",
        id, collection
      ))),
    }
  }

  #[allow(dead_code)]
  async fn cascade_remove_many_to_many_join<E: Entity>(
    &self,
    entity_id: &str,
    relation: &RelationDef,
  ) -> OrmResult<()> {
    let join_field = match &relation.join_field {
      Some(jf) => jf,
      None => return Ok(()),
    };

    let entity = self
      .provider
      .find_by_id(&E::table_name(), entity_id)
      .await?;

    let entity = match entity {
      Some(e) => e,
      None => return Ok(()),
    };

    let target_ids: Vec<String> =
      if let Some(arr) = entity.get(join_field).and_then(|v| v.as_array()) {
        arr
          .iter()
          .filter_map(|v| v.as_str().map(String::from))
          .collect()
      } else {
        return Ok(());
      };

    if target_ids.is_empty() {
      return Ok(());
    }

    let source_field = &relation.local_key;

    for target_id in target_ids {
      let target_doc = self
        .provider
        .find_by_id(&relation.target_collection, &target_id)
        .await?;

      if let Some(mut doc) = target_doc {
        if let Some(obj) = doc.as_object_mut() {
          if let Some(arr) = obj.get_mut(source_field).and_then(|v| v.as_array_mut()) {
            arr.retain(|v| v.as_str() != Some(entity_id));
            let patch = serde_json::json!({ source_field: arr });
            self
              .provider
              .patch(&relation.target_collection, &target_id, patch)
              .await?;
          }
        }
      }
    }

    Ok(())
  }

  async fn cascade_remove_many_to_many_join_by_collection(
    &self,
    entity_id: &str,
    collection: &str,
    relation: &RelationDef,
  ) -> OrmResult<()> {
    let join_field = match &relation.join_field {
      Some(jf) => jf,
      None => return Ok(()),
    };

    let entity = self.provider.find_by_id(collection, entity_id).await?;

    let entity = match entity {
      Some(e) => e,
      None => return Ok(()),
    };

    let target_ids: Vec<String> =
      if let Some(arr) = entity.get(join_field).and_then(|v| v.as_array()) {
        arr
          .iter()
          .filter_map(|v| v.as_str().map(String::from))
          .collect()
      } else {
        return Ok(());
      };

    if target_ids.is_empty() {
      return Ok(());
    }

    let source_field = &relation.local_key;

    for target_id in target_ids {
      let target_doc = self
        .provider
        .find_by_id(&relation.target_collection, &target_id)
        .await?;

      if let Some(mut doc) = target_doc {
        if let Some(obj) = doc.as_object_mut() {
          if let Some(arr) = obj.get_mut(source_field).and_then(|v| v.as_array_mut()) {
            arr.retain(|v| v.as_str() != Some(entity_id));
            let patch = serde_json::json!({ source_field: arr });
            self
              .provider
              .patch(&relation.target_collection, &target_id, patch)
              .await?;
          }
        }
      }
    }

    Ok(())
  }

  fn get_relations_for_collection(&self, collection: &str) -> Option<Vec<RelationDef>> {
    crate::relations::get_collection_relations(collection)
  }
}
