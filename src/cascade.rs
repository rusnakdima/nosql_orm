use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use crate::relations::{RelationDef, RelationType, WithRelations};
use crate::soft_delete::SoftDeletable;
use std::collections::HashSet;

fn cascade_value(entity_id: &str) -> serde_json::Value {
  serde_json::Value::String(entity_id.to_string())
}

fn insert_cascade_id(deleted_ids: &mut HashSet<String>, to_process: &mut Vec<String>, id: &str) {
  if !deleted_ids.contains(id) {
    deleted_ids.insert(id.to_string());
    to_process.push(id.to_string());
  }
}

pub struct CascadeManager<P: DatabaseProvider> {
  provider: P,
}

impl<P: DatabaseProvider> CascadeManager<P> {
  pub fn new(provider: P) -> Self {
    Self { provider }
  }

  pub async fn soft_delete_cascade<E: Entity + WithRelations + SoftDeletable>(
    &self,
    entity_id: &str,
    relations: &[RelationDef],
    deleted_ids: &mut HashSet<String>,
  ) -> OrmResult<bool> {
    if deleted_ids.contains(entity_id) {
      return Ok(true);
    }

    let exists = self.provider.exists(&E::table_name(), entity_id).await?;
    if !exists {
      return Ok(false);
    }

    self.soft_delete(&E::table_name(), entity_id).await?;

    let mut to_process = vec![entity_id.to_string()];
    insert_cascade_id(deleted_ids, &mut to_process, entity_id);

    while let Some(current_id) = to_process.pop() {
      self
        .process_soft_delete_cascade::<E>(&current_id, relations, deleted_ids, &mut to_process)
        .await?;
    }

    Ok(true)
  }

  async fn process_soft_delete_cascade<E: Entity + WithRelations + SoftDeletable>(
    &self,
    entity_id: &str,
    relations: &[RelationDef],
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<String>,
  ) -> OrmResult<()> {
    for rel in relations {
      if !self.should_cascade_soft_delete(rel) {
        continue;
      }

      match rel.relation_type {
        RelationType::OneToMany => {
          self
            .collect_cascade_soft_delete_one_to_many::<E>(entity_id, rel, deleted_ids, to_process)
            .await?;
        }
        RelationType::ManyToOne => {
          self
            .collect_cascade_soft_delete_many_to_one::<E>(entity_id, rel, deleted_ids, to_process)
            .await?;
        }
        RelationType::OneToOne => {
          self
            .collect_cascade_soft_delete_one_to_one::<E>(entity_id, rel, deleted_ids, to_process)
            .await?;
        }
        RelationType::ManyToMany => {
          self
            .cascade_remove_many_to_many_join::<E>(entity_id, rel)
            .await?;
        }
      }
    }
    Ok(())
  }

  pub async fn hard_delete_cascade<E: Entity + WithRelations>(
    &self,
    entity_id: &str,
    relations: &[RelationDef],
    deleted_ids: &mut HashSet<String>,
  ) -> OrmResult<bool> {
    if deleted_ids.contains(entity_id) {
      return Ok(true);
    }

    let existed = self.provider.delete(&E::table_name(), entity_id).await?;

    if existed {
      let mut to_process = vec![entity_id.to_string()];
      insert_cascade_id(deleted_ids, &mut to_process, entity_id);

      while let Some(current_id) = to_process.pop() {
        self
          .process_hard_delete_cascade::<E>(&current_id, relations, deleted_ids, &mut to_process)
          .await?;
      }
    }

    Ok(existed)
  }

  async fn process_hard_delete_cascade<E: Entity + WithRelations>(
    &self,
    entity_id: &str,
    relations: &[RelationDef],
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<String>,
  ) -> OrmResult<()> {
    for rel in relations {
      if !self.should_cascade_hard_delete(rel) {
        continue;
      }

      match rel.relation_type {
        RelationType::OneToMany => {
          self
            .collect_cascade_hard_delete_one_to_many::<E>(entity_id, rel, deleted_ids, to_process)
            .await?;
        }
        RelationType::ManyToOne => {
          self
            .collect_cascade_hard_delete_many_to_one::<E>(entity_id, rel, deleted_ids, to_process)
            .await?;
        }
        RelationType::OneToOne => {
          self
            .collect_cascade_hard_delete_one_to_one::<E>(entity_id, rel, deleted_ids, to_process)
            .await?;
        }
        RelationType::ManyToMany => {
          self
            .cascade_remove_many_to_many_join::<E>(entity_id, rel)
            .await?;
        }
      }
    }
    Ok(())
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

  async fn soft_delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let patch = serde_json::json!({ "deleted_at": chrono::Utc::now() });
    self.provider.patch(collection, id, patch).await?;
    Ok(true)
  }

  async fn collect_cascade_soft_delete_one_to_many<E: Entity + WithRelations + SoftDeletable>(
    &self,
    entity_id: &str,
    relation: &RelationDef,
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<String>,
  ) -> OrmResult<()> {
    let filter = Filter::Eq(relation.foreign_key.clone(), cascade_value(entity_id));

    let related = self
      .provider
      .find_many(
        &relation.target_collection,
        Some(&filter),
        None,
        None,
        None,
        true,
      )
      .await?;

    for doc in related {
      if let Some(id) = doc.get("id").and_then(|v| v.as_str()) {
        self.soft_delete(&relation.target_collection, id).await?;
        insert_cascade_id(deleted_ids, to_process, id);
      }
    }

    Ok(())
  }

  async fn collect_cascade_soft_delete_many_to_one<E: Entity + WithRelations + SoftDeletable>(
    &self,
    entity_id: &str,
    relation: &RelationDef,
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<String>,
  ) -> OrmResult<()> {
    let parent = self
      .provider
      .find_by_id(&E::table_name(), entity_id)
      .await?;

    let parent = match parent {
      Some(p) => p,
      None => return Ok(()),
    };

    if let Some(foreign_id) = parent.get(&relation.local_key).and_then(|v| v.as_str()) {
      self
        .soft_delete(&relation.target_collection, foreign_id)
        .await?;
      insert_cascade_id(deleted_ids, to_process, foreign_id);
    }

    Ok(())
  }

  async fn collect_cascade_soft_delete_one_to_one<E: Entity + WithRelations + SoftDeletable>(
    &self,
    entity_id: &str,
    relation: &RelationDef,
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<String>,
  ) -> OrmResult<()> {
    let parent = self
      .provider
      .find_by_id(&E::table_name(), entity_id)
      .await?;

    let parent = match parent {
      Some(p) => p,
      None => return Ok(()),
    };

    if let Some(foreign_id) = parent.get(&relation.local_key).and_then(|v| v.as_str()) {
      self
        .soft_delete(&relation.target_collection, foreign_id)
        .await?;
      insert_cascade_id(deleted_ids, to_process, foreign_id);
    }

    Ok(())
  }

  async fn collect_cascade_hard_delete_one_to_many<E: Entity + WithRelations>(
    &self,
    entity_id: &str,
    relation: &RelationDef,
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<String>,
  ) -> OrmResult<()> {
    let filter = Filter::Eq(relation.foreign_key.clone(), cascade_value(entity_id));

    let related = self
      .provider
      .find_many(
        &relation.target_collection,
        Some(&filter),
        None,
        None,
        None,
        true,
      )
      .await?;

    for doc in related {
      if let Some(id) = doc.get("id").and_then(|v| v.as_str()) {
        self
          .provider
          .delete(&relation.target_collection, id)
          .await?;
        insert_cascade_id(deleted_ids, to_process, id);
      }
    }

    Ok(())
  }

  async fn collect_cascade_hard_delete_many_to_one<E: Entity + WithRelations>(
    &self,
    entity_id: &str,
    relation: &RelationDef,
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<String>,
  ) -> OrmResult<()> {
    let parent = self
      .provider
      .find_by_id(&E::table_name(), entity_id)
      .await?;

    let parent = match parent {
      Some(p) => p,
      None => return Ok(()),
    };

    if let Some(foreign_id) = parent.get(&relation.local_key).and_then(|v| v.as_str()) {
      self
        .provider
        .delete(&relation.target_collection, foreign_id)
        .await?;
      insert_cascade_id(deleted_ids, to_process, foreign_id);
    }

    Ok(())
  }

  async fn collect_cascade_hard_delete_one_to_one<E: Entity + WithRelations>(
    &self,
    entity_id: &str,
    relation: &RelationDef,
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<String>,
  ) -> OrmResult<()> {
    let parent = self
      .provider
      .find_by_id(&E::table_name(), entity_id)
      .await?;

    let parent = match parent {
      Some(p) => p,
      None => return Ok(()),
    };

    if let Some(foreign_id) = parent.get(&relation.local_key).and_then(|v| v.as_str()) {
      self
        .provider
        .delete(&relation.target_collection, foreign_id)
        .await?;
      insert_cascade_id(deleted_ids, to_process, foreign_id);
    }

    Ok(())
  }

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

  async fn has_related_entities<E: Entity>(
    &self,
    entity_id: &str,
    relation: &RelationDef,
  ) -> OrmResult<bool> {
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
}
