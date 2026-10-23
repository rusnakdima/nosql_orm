use nosql_orm::entity::{Entity, EntityMeta};
use nosql_orm::error::{OrmError, OrmResult};
use nosql_orm::providers::json::JsonProvider;
use nosql_orm::relations::WithRelations;
use nosql_orm::repository::Repository;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestUser {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
  pub age: u32,
}

impl Entity for TestUser {
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }

  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }

  fn meta() -> EntityMeta {
    EntityMeta::new("users")
  }

  fn is_soft_deletable() -> bool {
    false
  }
}

impl WithRelations for TestUser {}

async fn create_test_repo() -> (Repository<TestUser, JsonProvider>, tempfile::TempDir) {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let repo = Repository::new(provider);
  (repo, temp_dir)
}

#[tokio::test]
async fn test_repository_find_by_id() {
  let (repo, _temp_dir) = create_test_repo().await;

  let user = repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  let found = repo.find_by_id(&user.id.clone().unwrap()).await.unwrap();
  assert!(found.is_some());
  assert_eq!(found.unwrap().name, "Alice");
}

#[tokio::test]
async fn test_repository_find_by_id_not_found() {
  let (repo, _temp_dir) = create_test_repo().await;

  let found = repo.find_by_id("nonexistent").await.unwrap();
  assert!(found.is_none());
}

#[tokio::test]
async fn test_repository_get_by_id() {
  let (repo, _temp_dir) = create_test_repo().await;

  let user = repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  let found = repo.get_by_id(&user.id.clone().unwrap()).await.unwrap();
  assert_eq!(found.name, "Alice");
}

#[tokio::test]
async fn test_repository_get_by_id_not_found() {
  let (repo, _temp_dir) = create_test_repo().await;

  let result = repo.get_by_id("nonexistent").await;
  assert!(result.is_err());
  assert!(matches!(result.unwrap_err(), OrmError::NotFound(_)));
}

#[tokio::test]
async fn test_repository_insert() {
  let (repo, _temp_dir) = create_test_repo().await;

  let user = repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  assert!(user.id.is_some());
  assert_eq!(user.name, "Alice");
}

#[tokio::test]
async fn test_repository_insert_with_id() {
  let (repo, _temp_dir) = create_test_repo().await;

  let user = repo
    .insert(TestUser {
      id: Some("custom-id".to_string()),
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  assert_eq!(user.id, Some("custom-id".to_string()));
}

#[tokio::test]
async fn test_repository_update() {
  let (repo, _temp_dir) = create_test_repo().await;

  let mut user = repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  user.age = 26;
  let updated = repo.update(user).await.unwrap();
  assert_eq!(updated.age, 26);

  let found = repo
    .find_by_id(&updated.id.unwrap())
    .await
    .unwrap()
    .unwrap();
  assert_eq!(found.age, 26);
}

#[tokio::test]
async fn test_repository_update_without_id() {
  let (repo, _temp_dir) = create_test_repo().await;

  let user = TestUser {
    id: None,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    age: 25,
  };

  let result = repo.update(user).await;
  assert!(result.is_err());
}

#[tokio::test]
async fn test_repository_save_insert() {
  let (repo, _temp_dir) = create_test_repo().await;

  let user = TestUser {
    id: None,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    age: 25,
  };

  let saved = repo.save(user).await.unwrap();
  assert!(saved.id.is_some());
}

#[tokio::test]
async fn test_repository_save_update() {
  let (repo, _temp_dir) = create_test_repo().await;

  let mut user = repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  user.age = 30;
  let saved = repo.save(user).await.unwrap();
  assert_eq!(saved.age, 30);
}

#[tokio::test]
async fn test_repository_find_all() {
  let (repo, _temp_dir) = create_test_repo().await;

  repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  repo
    .insert(TestUser {
      id: None,
      name: "Bob".to_string(),
      email: "bob@example.com".to_string(),
      age: 30,
    })
    .await
    .unwrap();

  let all = repo.find_all().await.unwrap();
  assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn test_repository_find_all_empty() {
  let (repo, _temp_dir) = create_test_repo().await;

  let all = repo.find_all().await.unwrap();
  assert!(all.is_empty());
}

#[tokio::test]
async fn test_repository_count() {
  let (repo, _temp_dir) = create_test_repo().await;

  assert_eq!(repo.count().await.unwrap(), 0);

  repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  repo
    .insert(TestUser {
      id: None,
      name: "Bob".to_string(),
      email: "bob@example.com".to_string(),
      age: 30,
    })
    .await
    .unwrap();

  assert_eq!(repo.count().await.unwrap(), 2);
}

#[tokio::test]
async fn test_repository_exists() {
  let (repo, _temp_dir) = create_test_repo().await;

  let user = repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  let id = user.id.unwrap();
  assert!(repo.exists(&id).await.unwrap());
  assert!(!repo.exists("nonexistent").await.unwrap());
}

#[tokio::test]
async fn test_repository_delete() {
  let (repo, _temp_dir) = create_test_repo().await;

  let user = repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  let id = user.id.unwrap();
  let deleted = repo.delete(&id).await.unwrap();
  assert!(deleted);

  let found = repo.find_by_id(&id).await.unwrap();
  assert!(found.is_none());
}

#[tokio::test]
async fn test_repository_delete_not_found() {
  let (repo, _temp_dir) = create_test_repo().await;

  let result = repo.delete("nonexistent").await.unwrap();
  // Deleting a missing id is not an error, it simply reports that nothing was removed.
  assert!(!result);
}

#[tokio::test]
async fn test_repository_delete_by_filter() {
  let (repo, _temp_dir) = create_test_repo().await;

  repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  repo
    .insert(TestUser {
      id: None,
      name: "Bob".to_string(),
      email: "bob@example.com".to_string(),
      age: 30,
    })
    .await
    .unwrap();

  let filter = nosql_orm::query::Filter::Eq("name".to_string(), serde_json::json!("Alice"));
  let deleted = repo.delete_many(Some(filter)).await.unwrap();
  assert_eq!(deleted, 1);

  let remaining = repo.find_all().await.unwrap();
  assert_eq!(remaining.len(), 1);
  assert_eq!(remaining[0].name, "Bob");
}

#[tokio::test]
async fn test_repository_find_many() {
  let (repo, _temp_dir) = create_test_repo().await;

  repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  repo
    .insert(TestUser {
      id: None,
      name: "Bob".to_string(),
      email: "bob@example.com".to_string(),
      age: 30,
    })
    .await
    .unwrap();

  let filter = nosql_orm::query::Filter::Eq("age".to_string(), serde_json::json!(25));
  let results = repo.query().filter(filter).find().await.unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0].name, "Alice");
}

#[tokio::test]
async fn test_repository_find_many_with_skip_limit() {
  let (repo, _temp_dir) = create_test_repo().await;

  for i in 0..10 {
    repo
      .insert(TestUser {
        id: None,
        name: format!("User{}", i),
        email: format!("user{}@example.com", i),
        age: 20 + i as u32,
      })
      .await
      .unwrap();
  }

  let results = repo.query().find().await.unwrap();
  assert_eq!(results.len(), 10);

  let results = repo.query().skip(5).limit(3).find().await.unwrap();
  assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_repository_find_many_with_projection() {
  let (repo, _temp_dir) = create_test_repo().await;

  repo
    .insert(TestUser {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 25,
    })
    .await
    .unwrap();

  let projection = nosql_orm::query::Projection::select(&["name"]);
  let doc = serde_json::json!({
    "id": "doc-1",
    "name": "Alice",
    "email": "alice@example.com",
    "age": 25
  });
  let projected = projection.apply(&doc);
  assert_eq!(
    projected.get("name").and_then(|v| v.as_str()),
    Some("Alice")
  );
  assert!(projected.get("email").is_none());
  assert!(projected.get("age").is_none());
  assert!(projected.get("id").is_none());

  let results = repo.query().select(&["name"]).find().await.unwrap();
  assert_eq!(results.len(), 1);
  assert_eq!(results[0].name, "Alice");
  assert_eq!(results[0].age, 25);
}

#[tokio::test]
async fn test_repository_new() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let repo = Repository::<TestUser, _>::new(provider);
  let count = repo.count().await.unwrap();
  assert_eq!(count, 0);
}

#[tokio::test]
async fn test_repository_with_timeout() {
  let temp_dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await.unwrap();
  let repo = Repository::<TestUser, _>::new(provider).with_timeout(5000);
  let count = repo.count().await.unwrap();
  assert_eq!(count, 0);
}

use nosql_orm::validators::Validate;

impl Validate for TestUser {
  fn validate(&self) -> OrmResult<()> {
    if self.name.is_empty() {
      return Err(OrmError::Validation("Name cannot be empty".to_string()));
    }
    Ok(())
  }
}
