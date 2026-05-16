use std::collections::HashSet;

use crate::entity::Entity;
use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use crate::relations::{RelationDef, RelationType, WithRelations};

use crate::cascade::helpers::{cascade_value, insert_cascade_id, CascadeEntityRef};

impl<P: DatabaseProvider> crate::CascadeManager<P> {
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
      let mut to_process = vec![CascadeEntityRef::new(entity_id, &E::table_name())];
      insert_cascade_id(deleted_ids, &mut to_process, entity_id, &E::table_name());

      while let Some(CascadeEntityRef {
        id: current_id,
        collection,
      }) = to_process.pop()
      {
        if let Some(entity_relations) = self.get_relations_for_collection(&collection) {
          self
            .process_hard_delete_cascade(
              &current_id,
              &collection,
              &entity_relations,
              deleted_ids,
              &mut to_process,
            )
            .await?;
        }
      }
    }

    Ok(existed)
  }

  async fn process_hard_delete_cascade(
    &self,
    entity_id: &str,
    collection: &str,
    relations: &[RelationDef],
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<CascadeEntityRef>,
  ) -> OrmResult<()> {
    for rel in relations {
      if !self.should_cascade_hard_delete(rel) {
        continue;
      }

      match rel.relation_type {
        RelationType::OneToMany => {
          self
            .collect_cascade_hard_delete_one_to_many(entity_id, rel, deleted_ids, to_process)
            .await?;
        }
        RelationType::ManyToOne | RelationType::OneToOne => {
          self
            .collect_cascade_single_side_hard(entity_id, collection, rel, deleted_ids, to_process)
            .await?;
        }
        RelationType::ManyToMany => {
          self
            .cascade_remove_many_to_many_join_by_collection(entity_id, collection, rel)
            .await?;
        }
      }
    }
    Ok(())
  }

  async fn collect_cascade_hard_delete_one_to_many(
    &self,
    entity_id: &str,
    relation: &RelationDef,
    deleted_ids: &mut HashSet<String>,
    to_process: &mut Vec<CascadeEntityRef>,
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
        insert_cascade_id(deleted_ids, to_process, id, &relation.target_collection);
      }
    }

    Ok(())
  }

  async fn collect_cascade_single_side_hard(
    &self,
    entity_id: &str,
    collection: &str,
    relation: &RelationDef,
    cascade_ids: &mut HashSet<String>,
    to_process: &mut Vec<CascadeEntityRef>,
  ) -> OrmResult<()> {
    let parent = self.provider.find_by_id(collection, entity_id).await?;

    let parent = match parent {
      Some(p) => p,
      None => return Ok(()),
    };

    if let Some(foreign_id) = parent.get(&relation.local_key).and_then(|v| v.as_str()) {
      self
        .provider
        .delete(&relation.target_collection, foreign_id)
        .await?;
      insert_cascade_id(
        cascade_ids,
        to_process,
        foreign_id,
        &relation.target_collection,
      );
    }

    Ok(())
  }
}
