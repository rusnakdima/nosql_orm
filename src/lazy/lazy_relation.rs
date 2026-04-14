use crate::entity::Entity;
use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::relations::RelationDef;
use crate::repository::Repository;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Lazy<T: Clone> {
  data: Arc<RwLock<Option<T>>>,
  loader: Arc<
    dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = OrmResult<T>> + Send + 'static>>
      + Send
      + Sync,
  >,
}

impl<T: Clone> Lazy<T> {
  pub fn new<F, Fut>(loader: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = OrmResult<T>> + Send + 'static,
  {
    Self {
      data: Arc::new(RwLock::new(None)),
      loader: Arc::new(Box::new(move || {
        Box::pin(loader()) as Pin<Box<dyn std::future::Future<Output = OrmResult<T>> + Send>>
      })),
    }
  }

  pub async fn get(&self) -> OrmResult<T> {
    {
      let read = self.data.read().await;
      if let Some(ref v) = *read {
        return Ok(v.clone());
      }
    }

    let value = (self.loader)().await?;
    {
      let mut write = self.data.write().await;
      *write = Some(value.clone());
    }
    Ok(value)
  }

  pub async fn is_loaded(&self) -> bool {
    let read = self.data.read().await;
    read.is_some()
  }

  pub async fn reload(&self) -> OrmResult<T> {
    let value = (self.loader)().await?;
    {
      let mut write = self.data.write().await;
      *write = Some(value.clone());
    }
    Ok(value)
  }
}

impl<T: Clone> Clone for Lazy<T> {
  fn clone(&self) -> Self {
    Self {
      data: self.data.clone(),
      loader: self.loader.clone(),
    }
  }
}

pub struct LazyRelation<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  repo: Repository<E, P>,
  relation: RelationDef,
  local_id: String,
  cached: Arc<RwLock<Option<Option<E>>>>,
}

impl<E, P> LazyRelation<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub fn new(repo: Repository<E, P>, relation: RelationDef, local_id: String) -> Self {
    Self {
      repo,
      relation,
      local_id,
      cached: Arc::new(RwLock::new(None)),
    }
  }

  pub async fn get(&self) -> OrmResult<Option<E>> {
    {
      let read = self.cached.read().await;
      if let Some(result) = &*read {
        return Ok(result.clone());
      }
    }

    let result = self.repo.find_by_id(&self.local_id).await;

    {
      let mut write = self.cached.write().await;
      *write = Some(result?);
    }

    let read = self.cached.read().await;
    Ok(read.clone().unwrap())
  }
}

pub struct LazyMany<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  repo: Repository<E, P>,
  relation: RelationDef,
  local_id: String,
  filter: Option<crate::query::Filter>,
  cached: Arc<RwLock<Option<Vec<E>>>>,
}

impl<E, P> LazyMany<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub fn new(repo: Repository<E, P>, relation: RelationDef, local_id: String) -> Self {
    Self {
      repo,
      relation,
      local_id,
      filter: None,
      cached: Arc::new(RwLock::new(None)),
    }
  }

  pub fn with_filter(mut self, filter: crate::query::Filter) -> Self {
    self.filter = Some(filter);
    self
  }

  pub async fn get(&self) -> OrmResult<Vec<E>> {
    {
      let read = self.cached.read().await;
      if let Some(result) = &*read {
        return Ok(result.clone());
      }
    }

    let mut query = self.repo.query();

    match self.relation.relation_type {
      crate::relations::RelationType::OneToMany => {
        query = query.where_eq(
          &self.relation.foreign_key,
          serde_json::json!(&self.local_id),
        );
      }
      crate::relations::RelationType::ManyToMany => {
        query = query.where_eq("ids", serde_json::json!([&self.local_id]));
      }
      _ => {}
    }

    if let Some(ref f) = self.filter {
      query = query.filter(f.clone());
    }

    let result = query.find().await?;

    {
      let mut write = self.cached.write().await;
      *write = Some(result.clone());
    }

    Ok(result)
  }

  pub async fn reload(&self) -> OrmResult<Vec<E>> {
    {
      let mut write = self.cached.write().await;
      *write = None;
    }
    self.get().await
  }
}
