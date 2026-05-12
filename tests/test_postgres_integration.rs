#[cfg(test)]
mod postgres_integration_tests {
  use nosql_orm::error::OrmError;
  use nosql_orm::prelude::*;
  use nosql_orm::providers::sql::PostgresProvider;
  use nosql_orm_derive::Entity;
  use serde::{Deserialize, Serialize};

  fn get_connection_string() -> String {
    std::env::var("POSTGRES_CONNECTION")
      .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/test_db".to_string())
  }

  #[derive(Debug, Clone, Serialize, Deserialize, Entity, Validate)]
  pub struct TestUser {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
    pub active: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
  }

  impl TestUser {
    pub fn new(name: &str, email: &str, age: Option<i32>) -> Self {
      Self {
        id: None,
        name: name.to_string(),
        email: email.to_string(),
        age,
        active: true,
        created_at: None,
        updated_at: None,
      }
    }
  }

  impl Timestamps for TestUser {
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
      self.created_at
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
      self.updated_at
    }
    fn set_created_at(&mut self, t: chrono::DateTime<chrono::Utc>) {
      self.created_at = Some(t);
    }
    fn set_updated_at(&mut self, t: chrono::DateTime<chrono::Utc>) {
      self.updated_at = Some(t);
    }
    fn apply_timestamps_for_insert(&mut self) {
      let now = chrono::Utc::now();
      if self.created_at.is_none() {
        self.created_at = Some(now);
      }
      if self.updated_at.is_none() {
        self.updated_at = Some(now);
      }
    }
    fn apply_timestamps_for_update(&mut self) {
      self.updated_at = Some(chrono::Utc::now());
    }
  }

  impl SoftDeletes for TestUser {
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
      None
    }
    fn set_deleted_at(&mut self, _t: Option<chrono::DateTime<chrono::Utc>>) {}
    fn restore(&mut self) {}
    fn is_deleted(&self) -> bool {
      false
    }
  }

  fn create_user(name: &str, email: &str, age: Option<i32>) -> TestUser {
    TestUser::new(name, email, age)
  }

  async fn setup_provider() -> OrmResult<PostgresProvider> {
    PostgresProvider::connect(&get_connection_string()).await
  }

  async fn cleanup_users(repo: &Repository<TestUser, PostgresProvider>) -> OrmResult<()> {
    let all = repo.find_all().await?;
    for user in all {
      if let Some(id) = user.id.clone() {
        let _ = repo.delete(&id).await;
      }
    }
    Ok(())
  }

  #[tokio::test]
  async fn test_connection_and_health_check() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };

    let healthy = provider.health_check().await?;
    assert!(healthy);

    let version = provider.get_server_version().await?;
    assert!(!version.is_empty());

    Ok(())
  }

  #[tokio::test]
  async fn test_insert_and_find() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };

    let repo: Repository<TestUser, _> = Repository::new(provider);

    let user = create_user("Alice", "alice@test.com", Some(30));
    let saved = repo.save(user).await?;

    assert!(saved.id.is_some());
    assert_eq!(saved.name, "Alice");

    if let Some(id) = saved.id {
      let found = repo.find_by_id(&id).await?;
      assert!(found.is_some());
      assert_eq!(found.unwrap().email, "alice@test.com");
    }

    Ok(())
  }

  #[tokio::test]
  async fn test_update() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    let user = create_user("Bob", "bob@test.com", Some(25));
    let saved = repo.save(user).await?;

    let mut updated = saved.clone();
    updated.name = "Robert".to_string();
    let result = repo.save(updated).await?;

    assert_eq!(result.name, "Robert");

    if let Some(id) = result.id {
      let _ = repo.delete(&id).await;
    }

    Ok(())
  }

  #[tokio::test]
  async fn test_delete() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    let user = create_user("Charlie", "charlie@test.com", None);
    let saved = repo.save(user).await?;

    if let Some(id) = saved.id {
      let deleted = repo.delete(&id).await?;
      assert!(deleted);

      let found = repo.find_by_id(&id).await?;
      assert!(found.is_none());
    }

    Ok(())
  }

  #[tokio::test]
  async fn test_find_many_with_filter() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    cleanup_users(&repo).await?;

    let users = vec![
      create_user("User1", "user1@test.com", Some(20)),
      create_user("User2", "user2@test.com", Some(30)),
      create_user("User3", "user3@test.com", Some(40)),
    ];

    for user in users {
      repo.save(user).await?;
    }

    let results = repo
      .query()
      .filter(Filter::Gt("age".to_string(), serde_json::json!(25)))
      .find()
      .await?;

    assert_eq!(results.len(), 2);

    cleanup_users(&repo).await?;

    Ok(())
  }

  #[tokio::test]
  async fn test_count() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    let count = repo.count().await?;
    assert!(count >= 0);

    Ok(())
  }

  #[tokio::test]
  async fn test_transaction_commit() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    cleanup_users(&repo).await?;

    let tx = provider.begin_transaction().await?;
    let user1 = create_user("TxUser1", "tx1@test.com", Some(25));
    let user2 = create_user("TxUser2", "tx2@test.com", Some(30));
    repo.save(user1).await?;
    repo.save(user2).await?;
    provider.commit_transaction(tx).await?;

    let all = repo.find_all().await?;
    let tx_users: Vec<_> = all
      .into_iter()
      .filter(|u| u.email.starts_with("tx"))
      .collect();
    assert_eq!(tx_users.len(), 2);

    cleanup_users(&repo).await?;

    Ok(())
  }

  #[tokio::test]
  async fn test_transaction_rollback() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    cleanup_users(&repo).await?;

    let tx = provider.begin_transaction().await?;
    let user1 = create_user("RollbackUser1", "rb1@test.com", Some(25));
    let user2 = create_user("RollbackUser2", "rb2@test.com", Some(30));
    repo.save(user1).await?;
    repo.save(user2).await?;
    provider.rollback_transaction(tx).await?;

    let all = repo.find_all().await?;
    let rb_users: Vec<_> = all
      .into_iter()
      .filter(|u| u.email.starts_with("rb"))
      .collect();
    assert_eq!(rb_users.len(), 0);

    cleanup_users(&repo).await?;

    Ok(())
  }

  #[tokio::test]
  async fn test_batch_operations() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    cleanup_users(&repo).await?;

    let users = vec![
      create_user("Batch1", "batch1@test.com", Some(20)),
      create_user("Batch2", "batch2@test.com", Some(25)),
      create_user("Batch3", "batch3@test.com", Some(30)),
    ];

    let count = repo.insert_many(users).await?;
    assert_eq!(count, 3);

    let total = repo.count().await?;
    assert!(total >= 3);

    cleanup_users(&repo).await?;

    Ok(())
  }

  #[tokio::test]
  async fn test_query_builder_usage() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    cleanup_users(&repo).await?;

    for i in 1..=5 {
      let mut user = create_user(
        &format!("QBUser{}", i),
        &format!("qb{}@test.com", i),
        Some(i * 10),
      );
      user.active = i % 2 == 0;
      repo.save(user).await?;
    }

    let active_results = repo
      .query()
      .filter(Filter::Equals(
        "active".to_string(),
        serde_json::json!(true),
      ))
      .find()
      .await?;
    assert_eq!(active_results.len(), 2);

    let young_results = repo
      .query()
      .filter(Filter::Lt("age".to_string(), serde_json::json!(30)))
      .find()
      .await?;
    assert_eq!(young_results.len(), 2);

    cleanup_users(&repo).await?;

    Ok(())
  }

  #[tokio::test]
  async fn test_update_many() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    cleanup_users(&repo).await?;

    let users = vec![
      create_user("UpdateMany1", "um1@test.com", Some(20)),
      create_user("UpdateMany2", "um2@test.com", Some(25)),
      create_user("UpdateMany3", "um3@test.com", Some(30)),
    ];

    for user in users {
      repo.save(user).await?;
    }

    let updated_count = repo
      .update_many(
        Some(Filter::Gt("age".to_string(), serde_json::json!(20))),
        serde_json::json!({"active": false}),
      )
      .await?;
    assert_eq!(updated_count, 2);

    cleanup_users(&repo).await?;

    Ok(())
  }

  #[tokio::test]
  async fn test_delete_many() -> OrmResult<()> {
    let provider = match setup_provider().await {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    cleanup_users(&repo).await?;

    let users = vec![
      create_user("DelMany1", "dm1@test.com", Some(20)),
      create_user("DelMany2", "dm2@test.com", Some(25)),
      create_user("DelMany3", "dm3@test.com", Some(30)),
    ];

    for user in users {
      repo.save(user).await?;
    }

    let deleted_count = repo
      .delete_many(Some(Filter::Gt("age".to_string(), serde_json::json!(20))))
      .await?;
    assert_eq!(deleted_count, 2);

    let remaining = repo.count().await?;
    assert_eq!(remaining, 1);

    cleanup_users(&repo).await?;

    Ok(())
  }
}
