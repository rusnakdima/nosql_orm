#![cfg(feature = "lazy")]

use nosql_orm::entity::{Entity, EntityMeta};
use nosql_orm::error::OrmResult;
use nosql_orm::lazy::lazy_loader::RepositoryLazyExt;
use nosql_orm::lazy::Lazy;
use nosql_orm::provider::Storage;
use nosql_orm::providers::json::JsonProvider;
use nosql_orm::relations::{RelationDef, RelationLoader, WithRelations};
use nosql_orm::repository::Repository;
use nosql_orm::validators::Validate;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyUser {
  pub id: Option<String>,
  pub name: String,
}

impl Entity for LazyUser {
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }

  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }

  fn meta() -> EntityMeta {
    EntityMeta::new("lazy_users")
  }
}

impl WithRelations for LazyUser {
  fn relations() -> Vec<RelationDef> {
    vec![RelationDef::one_to_many("posts", "lazy_posts", "author_id")]
  }
}

impl Validate for LazyUser {
  fn validate(&self) -> OrmResult<()> {
    Ok(())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyPost {
  pub id: Option<String>,
  pub title: String,
  pub author_id: String,
  pub tag_ids: Vec<String>,
}

impl Entity for LazyPost {
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }

  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }

  fn meta() -> EntityMeta {
    EntityMeta::new("lazy_posts")
  }
}

impl WithRelations for LazyPost {
  fn relations() -> Vec<RelationDef> {
    vec![
      RelationDef::many_to_one("author", "lazy_users", "author_id"),
      RelationDef::many_to_many("tags", "lazy_tags", "tag_ids"),
    ]
  }
}

impl Validate for LazyPost {
  fn validate(&self) -> OrmResult<()> {
    Ok(())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyTag {
  pub id: Option<String>,
  pub label: String,
}

impl Entity for LazyTag {
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }

  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }

  fn meta() -> EntityMeta {
    EntityMeta::new("lazy_tags")
  }
}

impl Validate for LazyTag {
  fn validate(&self) -> OrmResult<()> {
    Ok(())
  }
}

struct LazyFixture {
  provider: JsonProvider,
  user_repo: Repository<LazyUser, JsonProvider>,
  post_repo: Repository<LazyPost, JsonProvider>,
  tag_repo: Repository<LazyTag, JsonProvider>,
  _dir: tempfile::TempDir,
}

async fn fixture() -> LazyFixture {
  let dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();
  let user_repo = Repository::new(provider.clone());
  let post_repo = Repository::new(provider.clone());
  let tag_repo = Repository::new(provider.clone());
  LazyFixture {
    provider,
    user_repo,
    post_repo,
    tag_repo,
    _dir: dir,
  }
}

async fn seed(f: &LazyFixture) -> (String, String) {
  let user = f
    .user_repo
    .insert(LazyUser {
      id: None,
      name: "Alice".to_string(),
    })
    .await
    .unwrap();
  let post = f
    .post_repo
    .insert(LazyPost {
      id: None,
      title: "First".to_string(),
      author_id: user.id.clone().unwrap(),
      tag_ids: Vec::new(),
    })
    .await
    .unwrap();
  (user.id.unwrap(), post.id.unwrap())
}

#[tokio::test]
async fn lazy_load_resolves_single_relation_on_access() {
  let f = fixture().await;
  let (user_id, post_id) = seed(&f).await;

  let post = f.post_repo.find_by_id(&post_id).await.unwrap().unwrap();
  let handle = f
    .post_repo
    .lazy_load::<LazyUser>(&post.to_value().unwrap(), "author")
    .unwrap();

  assert!(!handle.is_loaded().await);
  let loaded = handle.get().await.unwrap();
  assert!(handle.is_loaded().await);
  assert_eq!((*loaded).as_ref().unwrap().name, "Alice");
  assert_eq!(
    (*loaded).as_ref().unwrap().id.as_deref(),
    Some(user_id.as_str())
  );
}

#[tokio::test]
async fn lazy_load_many_resolves_collection_on_access() {
  let f = fixture().await;
  let (user_id, _) = seed(&f).await;
  f.post_repo
    .insert(LazyPost {
      id: None,
      title: "Second".to_string(),
      author_id: user_id.clone(),
      tag_ids: Vec::new(),
    })
    .await
    .unwrap();

  let user = f.user_repo.find_by_id(&user_id).await.unwrap().unwrap();
  let handle = f
    .user_repo
    .lazy_load_many::<LazyPost>(&user.to_value().unwrap(), "posts")
    .unwrap();

  let posts = handle.get().await.unwrap();
  assert_eq!(posts.len(), 2);
  let mut titles: Vec<&str> = posts.iter().map(|p| p.title.as_str()).collect();
  titles.sort();
  assert_eq!(titles, vec!["First", "Second"]);
}

#[tokio::test]
async fn lazy_load_many_resolves_join_field_ids() {
  let f = fixture().await;
  let (_, post_id) = seed(&f).await;

  for (id, label) in [("tag-1", "rust"), ("tag-2", "orm"), ("tag-3", "json")] {
    f.tag_repo
      .insert(LazyTag {
        id: Some(id.to_string()),
        label: label.to_string(),
      })
      .await
      .unwrap();
  }

  let mut post = f.post_repo.find_by_id(&post_id).await.unwrap().unwrap();
  post.tag_ids = vec!["tag-1".to_string(), "tag-2".to_string()];
  f.post_repo.update(post.clone()).await.unwrap();

  let handle = f
    .post_repo
    .lazy_load_many::<LazyTag>(&post.to_value().unwrap(), "tags")
    .unwrap();
  let tags = handle.get().await.unwrap();
  assert_eq!(tags.len(), 2);
  let mut labels: Vec<&str> = tags.iter().map(|t| t.label.as_str()).collect();
  labels.sort();
  assert_eq!(labels, vec!["orm", "rust"]);
}

#[tokio::test]
async fn lazy_find_returns_cached_handle() {
  let f = fixture().await;
  let (user_id, _) = seed(&f).await;

  let handle = f.user_repo.lazy_find(user_id.clone());
  let first = handle.get().await.unwrap();
  assert!(first.is_some());

  // The value is cached: dropping the row does not invalidate the handle.
  f.provider.delete("lazy_users", &user_id).await.unwrap();
  let second = handle.get().await.unwrap();
  assert_eq!(
    (*second).as_ref().unwrap().id.as_deref(),
    Some(user_id.as_str())
  );
}

#[tokio::test]
async fn relation_loader_builds_lazy_handles() {
  let f = fixture().await;
  let (user_id, post_id) = seed(&f).await;

  let loader = RelationLoader::new(f.provider.clone());
  let post = f.post_repo.find_by_id(&post_id).await.unwrap().unwrap();

  let author_rel = RelationDef::many_to_one("author", "lazy_users", "author_id");
  let one = loader
    .lazy_one::<LazyUser>(&post.to_value().unwrap(), &author_rel)
    .unwrap();
  let author = one.get().await.unwrap();
  assert_eq!(
    (*author).as_ref().unwrap().id.as_deref(),
    Some(user_id.as_str())
  );

  let user = f.user_repo.find_by_id(&user_id).await.unwrap().unwrap();
  let posts_rel = RelationDef::one_to_many("posts", "lazy_posts", "author_id");
  let many = loader
    .lazy_many::<LazyPost>(&user.to_value().unwrap(), &posts_rel)
    .unwrap();
  let posts = many.get().await.unwrap();
  assert_eq!(posts.len(), 1);
}

#[tokio::test]
async fn lazy_load_rejects_wrong_relation_shape() {
  let f = fixture().await;
  let (user_id, post_id) = seed(&f).await;

  let post = f.post_repo.find_by_id(&post_id).await.unwrap().unwrap();
  // Unknown relation name.
  assert!(f
    .post_repo
    .lazy_load::<LazyUser>(&post.to_value().unwrap(), "does_not_exist")
    .is_err());

  let user = f.user_repo.find_by_id(&user_id).await.unwrap().unwrap();
  // A collection relation must not produce a single-value handle.
  assert!(f
    .user_repo
    .lazy_load::<LazyPost>(&user.to_value().unwrap(), "posts")
    .is_err());
  // A single-value relation must not produce a collection handle.
  assert!(f
    .post_repo
    .lazy_load_many::<LazyUser>(&post.to_value().unwrap(), "author")
    .is_err());
}

#[tokio::test]
async fn generic_lazy_memoizes_loader_calls() {
  let calls = Arc::new(AtomicUsize::new(0));
  let counter = calls.clone();
  let handle: Lazy<u32> = Lazy::new(move || {
    let counter = counter.clone();
    async move {
      counter.fetch_add(1, Ordering::SeqCst);
      Ok(7)
    }
  });

  assert!(!handle.is_loaded().await);
  assert_eq!(*handle.get().await.unwrap(), 7);
  assert_eq!(*handle.get().await.unwrap(), 7);
  assert_eq!(calls.load(Ordering::SeqCst), 1);
  assert!(handle.is_loaded().await);
}

#[tokio::test]
async fn repository_lazy_ext_still_builds_handles() {
  let f = fixture().await;
  let (user_id, _) = seed(&f).await;

  let arc: Arc<Repository<LazyPost, JsonProvider>> = Arc::new(Repository::new(f.provider.clone()));
  let rel = RelationDef::one_to_many("posts", "lazy_posts", "author_id");
  let handle = arc.clone().lazy_many(rel, user_id.clone());
  let posts = handle.get().await.unwrap();
  assert_eq!(posts.len(), 1);
}
