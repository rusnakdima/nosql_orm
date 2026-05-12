use crate::entity::Entity;
use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::relations::RelationDef;
use crate::repository::Repository;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

type LoaderFuture<T> =
  std::pin::Pin<Box<dyn std::future::Future<Output = OrmResult<T>> + Send + 'static>>;
type LoaderFn<T> = Arc<dyn Fn() -> LoaderFuture<T> + Send + Sync>;

pub struct Lazy<T> {
  data: Arc<RwLock<Option<Arc<T>>>>,
  loader: LoaderFn<T>,
}

impl<T> Lazy<T> {
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

  pub async fn get(&self) -> OrmResult<Arc<T>> {
    {
      let read = self.data.read().await;
      if let Some(ref v) = *read {
        return Ok(v.clone());
      }
    }

    let value = Arc::new((self.loader)().await?);
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

  pub async fn reload(&self) -> OrmResult<Arc<T>> {
    let value = Arc::new((self.loader)().await?);
    {
      let mut write = self.data.write().await;
      *write = Some(value.clone());
    }
    Ok(value)
  }
}

impl<T> Clone for Lazy<T> {
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
  repo: Weak<Repository<E, P>>,
  local_id: String,
  cached: Arc<RwLock<Option<Arc<Option<E>>>>>,
}

impl<E, P> LazyRelation<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub fn new(repo: Arc<Repository<E, P>>, _relation: RelationDef, local_id: String) -> Self {
    Self {
      repo: Arc::downgrade(&repo),
      local_id,
      cached: Arc::new(RwLock::new(None)),
    }
  }

  pub async fn get(&self) -> OrmResult<Arc<Option<E>>> {
    {
      let read = self.cached.read().await;
      if let Some(result) = &*read {
        return Ok(result.clone());
      }
    }

    let repo = self
      .repo
      .upgrade()
      .ok_or_else(|| crate::error::OrmError::Internal("Repository has been dropped".to_string()))?;
    let result = repo.find_by_id(&self.local_id).await;

    {
      let mut write = self.cached.write().await;
      *write = Some(Arc::new(result?));
    }

    let read = self.cached.read().await;
    Ok(read.clone().unwrap())
  }

  pub fn close(&self) {
    let mut write = self.cached.blocking_write();
    *write = None;
  }
}

pub struct LazyMany<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  repo: Weak<Repository<E, P>>,
  relation: RelationDef,
  local_id: String,
  filter: Option<crate::query::Filter>,
  cached: Arc<RwLock<Option<Arc<Vec<E>>>>>,
}

impl<E, P> LazyMany<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub fn new(repo: Arc<Repository<E, P>>, relation: RelationDef, local_id: String) -> Self {
    Self {
      repo: Arc::downgrade(&repo),
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

  pub async fn get(&self) -> OrmResult<Arc<Vec<E>>> {
    {
      let read = self.cached.read().await;
      if let Some(result) = &*read {
        return Ok(result.clone());
      }
    }

    let repo = self
      .repo
      .upgrade()
      .ok_or_else(|| crate::error::OrmError::Internal("Repository has been dropped".to_string()))?;
    let mut query = repo.query();

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
      *write = Some(Arc::new(result));
    }

    let read = self.cached.read().await;
    Ok(read.clone().unwrap())
  }

  pub async fn reload(&self) -> OrmResult<Arc<Vec<E>>> {
    {
      let mut write = self.cached.write().await;
      *write = None;
    }
    self.get().await
  }

  pub fn close(&self) {
    let mut write = self.cached.blocking_write();
    *write = None;
  }
}
