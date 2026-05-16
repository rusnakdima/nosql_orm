use chrono::{DateTime, Utc};
use nosql_orm::soft_delete::{SoftDeletable, SoftDeleteExt};

#[test]
fn test_soft_deletable_option_datetime() {
  let deleted_at: Option<DateTime<Utc>> = None;
  assert!(!deleted_at.is_deleted());
  assert_eq!(deleted_at.deleted_at(), None);

  let mut deleted_at: Option<DateTime<Utc>> = None;
  deleted_at.mark_deleted();
  assert!(deleted_at.is_deleted());
  assert!(deleted_at.deleted_at().is_some());

  let mut deleted_at: Option<DateTime<Utc>> = Some(Utc::now());
  deleted_at.restore();
  assert!(!deleted_at.is_deleted());
  assert_eq!(deleted_at.deleted_at(), None);
}

#[test]
fn test_soft_deletable_soft_delete_ext() {
  let ext = SoftDeleteExt { deleted_at: None };
  assert!(!ext.is_deleted());
  assert_eq!(ext.deleted_at(), None);

  let mut ext = SoftDeleteExt { deleted_at: None };
  ext.mark_deleted();
  assert!(ext.is_deleted());
  assert!(ext.deleted_at().is_some());

  let mut ext = SoftDeleteExt {
    deleted_at: Some(Utc::now()),
  };
  ext.restore();
  assert!(!ext.is_deleted());
  assert_eq!(ext.deleted_at(), None);
}

#[test]
fn test_soft_delete_ext_clone() {
  let ext = SoftDeleteExt {
    deleted_at: Some(Utc::now()),
  };
  let cloned = ext.clone();
  assert_eq!(cloned.deleted_at(), ext.deleted_at());
}

#[test]
fn test_soft_delete_ext_debug() {
  let ext = SoftDeleteExt { deleted_at: None };
  let debug_str = format!("{:?}", ext);
  assert!(debug_str.contains("SoftDeleteExt"));

  let ext_with_date = SoftDeleteExt {
    deleted_at: Some(Utc::now()),
  };
  let debug_str = format!("{:?}", ext_with_date);
  assert!(debug_str.contains("SoftDeleteExt"));
}

#[test]
fn test_mark_deleted_sets_current_time() {
  let mut deleted_at: Option<DateTime<Utc>> = None;
  let before = Utc::now();
  deleted_at.mark_deleted();
  let after = Utc::now();

  let deleted = deleted_at.deleted_at().unwrap();
  assert!(deleted >= before && deleted <= after);
}

#[test]
fn test_restore_clears_deleted_at() {
  let mut deleted_at: Option<DateTime<Utc>> = Some(Utc::now());
  assert!(deleted_at.is_deleted());
  deleted_at.restore();
  assert!(!deleted_at.is_deleted());
  assert!(deleted_at.deleted_at().is_none());
}

#[test]
fn test_is_deleted_returns_true_when_deleted() {
  let deleted_at: Option<DateTime<Utc>> = Some(Utc::now());
  assert!(deleted_at.is_deleted());
}

#[test]
fn test_is_deleted_returns_false_when_not_deleted() {
  let deleted_at: Option<DateTime<Utc>> = None;
  assert!(!deleted_at.is_deleted());
}

#[test]
fn test_soft_delete_ext_mark_deleted() {
  let mut ext = SoftDeleteExt { deleted_at: None };
  ext.mark_deleted();
  assert!(ext.deleted_at().is_some());
}

#[test]
fn test_soft_delete_ext_restore() {
  let mut ext = SoftDeleteExt {
    deleted_at: Some(Utc::now()),
  };
  ext.restore();
  assert!(ext.deleted_at().is_none());
}

#[test]
fn test_set_deleted_at() {
  let mut deleted_at: Option<DateTime<Utc>> = None;
  let time = Utc::now();
  deleted_at.set_deleted_at(Some(time));
  assert_eq!(deleted_at.deleted_at(), Some(time));

  deleted_at.set_deleted_at(None);
  assert_eq!(deleted_at.deleted_at(), None);
}

#[test]
fn test_soft_delete_ext_set_deleted_at() {
  let mut ext = SoftDeleteExt { deleted_at: None };
  let time = Utc::now();
  ext.set_deleted_at(Some(time));
  assert_eq!(ext.deleted_at(), Some(time));
}

use nosql_orm::entity::Entity;
use nosql_orm::error::OrmError;
use nosql_orm::providers::json::JsonProvider;
use nosql_orm::repository::Repository;
use nosql_orm::timestamps::Timestamps;
use nosql_orm::validators::Validate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftDeletedUser {
  pub id: Option<String>,
  pub name: String,
  #[serde(default)]
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for SoftDeletedUser {
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }

  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }

  fn collection() -> &'static str {
    "soft_deleted_users"
  }

  fn is_soft_deletable() -> bool {
    true
  }
}

impl Timestamps for SoftDeletedUser {}

impl Validate for SoftDeletedUser {
  fn validate(&self) -> OrmResult<()> {
    if self.name.is_empty() {
      return Err(OrmError::Validation("Name cannot be empty".to_string()));
    }
    Ok(())
  }
}

impl SoftDeletable for SoftDeletedUser {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }

  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    self.deleted_at = deleted_at;
  }
}

fn create_soft_delete_repo() -> (Repository<SoftDeletedUser, JsonProvider>, tempfile::TempDir) {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).unwrap();
  let repo = Repository::new(provider);
  (repo, temp_dir)
}

#[tokio::test]
async fn test_soft_delete_repository_find_all_excludes_deleted() {
  let (repo, _temp_dir) = create_soft_delete_repo();

  repo
    .insert(SoftDeletedUser {
      id: None,
      name: "Active User".to_string(),
      deleted_at: None,
    })
    .await
    .unwrap();

  repo
    .insert(SoftDeletedUser {
      id: None,
      name: "Deleted User".to_string(),
      deleted_at: Some(Utc::now()),
    })
    .await
    .unwrap();

  let all = repo.find_all().await.unwrap();
  assert_eq!(all.len(), 1);
  assert_eq!(all[0].name, "Active User");
}

#[tokio::test]
async fn test_soft_delete_repository_find_all_including_deleted() {
  let (repo, _temp_dir) = create_soft_delete_repo();

  repo
    .insert(SoftDeletedUser {
      id: None,
      name: "Active User".to_string(),
      deleted_at: None,
    })
    .await
    .unwrap();

  repo
    .insert(SoftDeletedUser {
      id: None,
      name: "Deleted User".to_string(),
      deleted_at: Some(Utc::now()),
    })
    .await
    .unwrap();

  let all = repo.find_all_including_deleted().await.unwrap();
  assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn test_soft_delete_repository_find_by_id_excludes_deleted() {
  let (repo, _temp_dir) = create_soft_delete_repo();

  let user = repo
    .insert(SoftDeletedUser {
      id: None,
      name: "User".to_string(),
      deleted_at: None,
    })
    .await
    .unwrap();

  let id = user.id.unwrap();

  let found = repo.find_by_id(&id).await.unwrap();
  assert!(found.is_some());

  repo.hard_delete(&id).await.unwrap();

  let found = repo.find_by_id(&id).await.unwrap();
  assert!(found.is_none());
}

#[tokio::test]
async fn test_soft_delete_repository_restore() {
  let (repo, _temp_dir) = create_soft_delete_repo();

  let mut user = repo
    .insert(SoftDeletedUser {
      id: None,
      name: "User".to_string(),
      deleted_at: None,
    })
    .await
    .unwrap();

  let id = user.id.unwrap();

  user.deleted_at = Some(Utc::now());
  repo.update(user).await.unwrap();

  let found = repo.find_by_id(&id).await.unwrap();
  assert!(found.is_none());

  repo.restore(&id).await.unwrap();

  let found = repo.find_by_id(&id).await.unwrap();
  assert!(found.is_some());
}

#[tokio::test]
async fn test_soft_delete_repository_hard_delete() {
  let (repo, _temp_dir) = create_soft_delete_repo();

  let user = repo
    .insert(SoftDeletedUser {
      id: None,
      name: "User".to_string(),
      deleted_at: None,
    })
    .await
    .unwrap();

  let id = user.id.unwrap();
  let deleted = repo.hard_delete(&id).await.unwrap();
  assert!(deleted);

  let found = repo.find_by_id(&id).await.unwrap();
  assert!(found.is_none());
}

#[tokio::test]
async fn test_soft_delete_repository_soft_delete() {
  let (repo, _temp_dir) = create_soft_delete_repo();

  let user = repo
    .insert(SoftDeletedUser {
      id: None,
      name: "User".to_string(),
      deleted_at: None,
    })
    .await
    .unwrap();

  let id = user.id.unwrap();
  repo.soft_delete(&id).await.unwrap();

  let found = repo.find_by_id(&id).await.unwrap();
  assert!(found.is_none());

  let all = repo.find_all_including_deleted().await.unwrap();
  assert_eq!(all.len(), 1);
  assert!(all[0].deleted_at.is_some());
}

#[tokio::test]
async fn test_soft_delete_repository_delete_marks_as_deleted() {
  let (repo, _temp_dir) = create_soft_delete_repo();

  let user = repo
    .insert(SoftDeletedUser {
      id: None,
      name: "User".to_string(),
      deleted_at: None,
    })
    .await
    .unwrap();

  let id = user.id.unwrap();
  repo.delete(&id).await.unwrap();

  let all = repo.find_all_including_deleted().await.unwrap();
  assert_eq!(all.len(), 1);
  assert!(all[0].deleted_at.is_some());
}

use nosql_orm::query::Filter;

#[tokio::test]
async fn test_soft_delete_repository_find_soft_deleted() {
  let (repo, _temp_dir) = create_soft_delete_repo();

  repo
    .insert(SoftDeletedUser {
      id: None,
      name: "Active".to_string(),
      deleted_at: None,
    })
    .await
    .unwrap();

  repo
    .insert(SoftDeletedUser {
      id: None,
      name: "Deleted".to_string(),
      deleted_at: Some(Utc::now()),
    })
    .await
    .unwrap();

  let filter = Filter::IsNotNull("deleted_at".to_string());
  let results = repo.find_many(Some(filter)).await.unwrap();
  assert_eq!(results.len(), 0);
}

#[tokio::test]
async fn test_soft_delete_repository_count_only_active() {
  let (repo, _temp_dir) = create_soft_delete_repo();

  repo
    .insert(SoftDeletedUser {
      id: None,
      name: "Active".to_string(),
      deleted_at: None,
    })
    .await
    .unwrap();

  repo
    .insert(SoftDeletedUser {
      id: None,
      name: "Deleted".to_string(),
      deleted_at: Some(Utc::now()),
    })
    .await
    .unwrap();

  assert_eq!(repo.count().await.unwrap(), 1);
}

#[tokio::test]
async fn test_soft_delete_entity_trait() {
  let mut user = SoftDeletedUser {
    id: Some("123".to_string()),
    name: "Test".to_string(),
    deleted_at: None,
  };

  assert!(!user.is_deleted());
  user.mark_deleted();
  assert!(user.is_deleted());
  assert!(user.deleted_at().is_some());

  user.restore();
  assert!(!user.is_deleted());
  assert!(user.deleted_at().is_none());
}
