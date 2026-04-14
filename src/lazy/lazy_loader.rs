use crate::entity::Entity;
use crate::lazy::lazy_relation::{LazyMany, LazyRelation};
use crate::provider::DatabaseProvider;
use crate::relations::RelationDef;
use crate::repository::Repository;

pub struct LazyLoader;

impl LazyLoader {
  pub fn relation<E, P>(
    repo: Repository<E, P>,
    relation: RelationDef,
    local_id: String,
  ) -> LazyRelation<E, P>
  where
    E: Entity,
    P: DatabaseProvider,
  {
    LazyRelation::new(repo, relation, local_id)
  }

  pub fn many<E, P>(
    repo: Repository<E, P>,
    relation: RelationDef,
    local_id: String,
  ) -> LazyMany<E, P>
  where
    E: Entity,
    P: DatabaseProvider,
  {
    LazyMany::new(repo, relation, local_id)
  }
}

pub trait RepositoryLazyExt<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  fn lazy_relation(&self, relation: RelationDef, local_id: String) -> LazyRelation<E, P>;
  fn lazy_many(&self, relation: RelationDef, local_id: String) -> LazyMany<E, P>;
}

impl<E, P> RepositoryLazyExt<E, P> for Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  fn lazy_relation(&self, relation: RelationDef, local_id: String) -> LazyRelation<E, P> {
    LazyLoader::relation(self.clone(), relation, local_id)
  }

  fn lazy_many(&self, relation: RelationDef, local_id: String) -> LazyMany<E, P> {
    LazyLoader::many(self.clone(), relation, local_id)
  }
}
