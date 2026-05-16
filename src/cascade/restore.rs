use std::collections::HashSet;

use crate::entity::Entity;
use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use crate::relations::{RelationDef, RelationType, WithRelations};
use crate::soft_delete::SoftDeletable;

use crate::cascade::helpers::{cascade_value, insert_cascade_id, CascadeEntityRef};

impl<P: DatabaseProvider> crate::CascadeManager<P> {
  pub async fn restore_cascade<E: Entity + WithRelations + SoftDeletable>(
    &self,
    entity_id: &str,
    _relations: &[RelationDef],
    restored_ids: &mut HashSet<String>,
  ) -> OrmResult<bool> {
    if restored_ids.contains(entity_id) {
      return Ok(true);
    }

    let exists = self.provider.exists(&E::table_name(), entity_id).await?;
    if !exists {
      return Ok(false);
    }

    self.restore(&E::table_name(), entity_id).await?;

    let mut to_process = vec![CascadeEntityRef::new(entity_id, &E::table_name())];
    insert_cascade_id(restored_ids, &mut to_process, entity_id, &E::table_name());

    while let Some(CascadeEntityRef {
      id: current_id,
      collection,
    }) = to_process.pop()
    {
      if let Some(entity_relations) = self.get_relations_for_collection(&collection) {
        self
          .process_restore_cascade(
            &current_id,
            &collection,
            &entity_relations,
            restored_ids,
            &mut to_process,
          )
          .await?;
      }
    }

    Ok(true)
  }

  async fn process_restore_cascade(
    &self,
    entity_id: &str,
    collection: &str,
    relations: &[RelationDef],
    restored_ids: &mut HashSet<String>,
    to_process: &mut Vec<CascadeEntityRef>,
  ) -> OrmResult<()> {
    for rel in relations {
      if !self.should_cascade_soft_delete(rel) {
        continue;
      }

      match rel.relation_type {
        RelationType::OneToMany => {
          self
            .collect_restore_one_to_many(entity_id, rel, restored_ids, to_process)
            .await?;
        }
        RelationType::ManyToOne | RelationType::OneToOne => {
          self
            .collect_cascade_single_side(
              entity_id,
              collection,
              rel,
              crate::cascade::helpers::CascadeAction::Restore,
              restored_ids,
              to_process,
            )
            .await?;
        }
        RelationType::ManyToMany => {}
      }
    }
    Ok(())
  }

  async fn collect_restore_one_to_many(
    &self,
    entity_id: &str,
    relation: &RelationDef,
    restored_ids: &mut HashSet<String>,
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
        self.restore(&relation.target_collection, id).await?;
        insert_cascade_id(restored_ids, to_process, id, &relation.target_collection);
      }
    }

    Ok(())
  }
}
