use std::collections::HashSet;

use crate::entity::Entity;
use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use crate::relations::{RelationDef, RelationType, WithRelations};

use crate::cascade::helpers::{cascade_value, insert_cascade_id};

impl<P: DatabaseProvider> crate::CascadeManager<P> {
  pub async fn soft_delete_cascade<
    E: Entity + WithRelations + crate::soft_delete::SoftDeletable,
  >(
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

  async fn process_soft_delete_cascade<
    E: Entity + WithRelations + crate::soft_delete::SoftDeletable,
  >(
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
            .collect_cascade_soft_delete_one_to_many(entity_id, rel, deleted_ids, to_process)
            .await?;
        }
        RelationType::ManyToOne | RelationType::OneToOne => {
          self
            .collect_cascade_single_side::<E>(
              entity_id,
              rel,
              crate::cascade::helpers::CascadeAction::SoftDelete,
              deleted_ids,
              to_process,
            )
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

  async fn collect_cascade_soft_delete_one_to_many(
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

  pub(super) async fn collect_cascade_single_side<
    E: Entity + WithRelations + crate::soft_delete::SoftDeletable,
  >(
    &self,
    entity_id: &str,
    relation: &RelationDef,
    action: crate::cascade::helpers::CascadeAction,
    cascade_ids: &mut HashSet<String>,
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
      match action {
        crate::cascade::helpers::CascadeAction::SoftDelete => {
          self
            .soft_delete(&relation.target_collection, foreign_id)
            .await?;
        }
        crate::cascade::helpers::CascadeAction::Restore => {
          self
            .restore(&relation.target_collection, foreign_id)
            .await?;
        }
      }
      insert_cascade_id(cascade_ids, to_process, foreign_id);
    }

    Ok(())
  }
}
