use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::{QueryBuilder, SortDirection};
use crate::relations::{RelationLoader, RelationType, RelationValue, WithLoaded, WithRelations};
use crate::soft_delete::SoftDeletable;
use std::collections::HashSet;

use super::Repository;

pub struct RelationRepository<E, P>
where
  E: WithRelations,
  P: DatabaseProvider,
{
  inner: Repository<E, P>,
  loader: RelationLoader<P>,
}

impl<E, P> RelationRepository<E, P>
where
  E: WithRelations,
  P: DatabaseProvider,
{
  pub fn new(provider: P) -> Self {
    let loader = RelationLoader::new(provider.clone());
    Self {
      inner: Repository::new(provider),
      loader,
    }
  }

  pub fn repo(&self) -> &Repository<E, P> {
    &self.inner
  }
}

impl<E, P> RelationRepository<E, P>
where
  E: WithRelations + SoftDeletable,
  P: DatabaseProvider,
{
  pub async fn soft_delete_cascade(&self, id: impl AsRef<str>) -> OrmResult<bool> {
    self.inner.soft_delete(id).await
  }

  pub async fn find_with_relations(
    &self,
    id: impl AsRef<str>,
    relation_paths: &[&str],
  ) -> OrmResult<Option<WithLoaded<E>>> {
    let entity = match self.inner.find_by_id(id).await? {
      Some(e) => e,
      None => return Ok(None),
    };

    let mut result = WithLoaded::new(entity);
    let mut doc = result.entity.to_value()?;

    let mut visited = HashSet::new();
    visited.insert(E::table_name());

    for path in relation_paths {
      if let Some(rel_def) = E::relations()
        .iter()
        .find(|r| r.name.as_str() == *path)
        .cloned()
      {
        let enriched = self
          .loader
          .load_relation_recursive(vec![doc.clone()], &rel_def, &mut visited)
          .await?;

        if let Some(updated) = enriched.into_iter().next() {
          doc = updated;
          result.entity = E::from_value(doc.clone())?;

          if let Some(rel_val) = doc.get(*path) {
            match rel_def.relation_type {
              RelationType::ManyToOne | RelationType::OneToOne => {
                result.loaded.insert(
                  path.to_string(),
                  RelationValue::Single(Some(rel_val.clone())),
                );
              }
              RelationType::OneToMany | RelationType::ManyToMany => {
                if let Some(arr) = rel_val.as_array() {
                  result
                    .loaded
                    .insert(path.to_string(), RelationValue::Many(arr.clone()));
                } else {
                  result
                    .loaded
                    .insert(path.to_string(), RelationValue::Many(vec![]));
                }
              }
            }
          }
        }
      }
    }

    Ok(Some(result))
  }

  pub async fn find_all_with_relations(
    &self,
    relation_paths: &[&str],
  ) -> OrmResult<Vec<WithLoaded<E>>> {
    let entities = self.inner.find_all().await?;

    let mut results = Vec::with_capacity(entities.len());

    for entity in entities {
      let mut result = WithLoaded::new(entity);
      let mut doc = result.entity.to_value()?;

      let mut visited = HashSet::new();
      visited.insert(E::table_name());

      for path in relation_paths {
        let rel_name = path.split('.').next().unwrap_or(path);

        if let Some(rel_def) = E::relations()
          .iter()
          .find(|r| r.name.as_str() == rel_name)
          .cloned()
        {
          let enriched = self
            .loader
            .load_relation_recursive(vec![doc.clone()], &rel_def, &mut visited)
            .await?;

          if let Some(updated) = enriched.into_iter().next() {
            if let Some(rel_val) = updated.get(rel_name) {
              match rel_def.relation_type {
                RelationType::ManyToOne | RelationType::OneToOne => {
                  result.loaded.insert(
                    rel_name.to_string(),
                    RelationValue::Single(Some(rel_val.clone())),
                  );
                }
                RelationType::OneToMany | RelationType::ManyToMany => {
                  if let Some(arr) = rel_val.as_array() {
                    result
                      .loaded
                      .insert(rel_name.to_string(), RelationValue::Many(arr.clone()));
                  }
                }
              }
            }
            doc = updated;
          }
        }
      }

      results.push(result);
    }

    Ok(results)
  }

  pub async fn query_with_relations(
    &self,
    builder: QueryBuilder,
    relation_paths: &[&str],
  ) -> OrmResult<Vec<WithLoaded<E>>> {
    let filter = builder.build_filter();
    let (sort_field, sort_asc) = match &builder.order {
      Some(o) => (Some(o.field.clone()), o.direction == SortDirection::Asc),
      None => (None, true),
    };
    let docs = self
      .inner
      .provider
      .find_many(
        &E::table_name(),
        filter.as_ref(),
        builder.skip,
        builder.limit,
        sort_field.as_deref(),
        sort_asc,
      )
      .await?;

    let mut results = Vec::with_capacity(docs.len());

    for doc in docs {
      let entity = E::from_value(doc.clone())?;
      let mut result = WithLoaded::new(entity);
      let mut doc_for_loader = doc.clone();

      let mut visited = HashSet::new();
      visited.insert(E::table_name());

      for path in relation_paths {
        let rel_name = path.split('.').next().unwrap_or(path);

        if let Some(rel_def) = E::relations()
          .iter()
          .find(|r| r.name.as_str() == rel_name)
          .cloned()
        {
          let enriched = self
            .loader
            .load_relation_recursive(vec![doc_for_loader.clone()], &rel_def, &mut visited)
            .await?;

          if let Some(updated) = enriched.into_iter().next() {
            if let Some(rel_val) = updated.get(rel_name) {
              match rel_def.relation_type {
                RelationType::ManyToOne | RelationType::OneToOne => {
                  result.loaded.insert(
                    rel_name.to_string(),
                    RelationValue::Single(Some(rel_val.clone())),
                  );
                }
                RelationType::OneToMany | RelationType::ManyToMany => {
                  if let Some(arr) = rel_val.as_array() {
                    result
                      .loaded
                      .insert(rel_name.to_string(), RelationValue::Many(arr.clone()));
                  }
                }
              }
            }
            doc_for_loader = updated;
          }
        }
      }

      results.push(result);
    }

    Ok(results)
  }
}
