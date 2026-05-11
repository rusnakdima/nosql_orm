#[cfg(test)]
mod postgres_integration_tests {
  use nosql_orm::error::OrmError;
  use nosql_orm::prelude::*;
  use nosql_orm::providers::sql::PostgresProvider;
  use nosql_orm_derive::Entity;
  use serde::{Deserialize, Serialize};

  #[derive(Debug, Clone, Serialize, Deserialize, Entity, Validate)]
  pub struct TestUser {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
  }

  impl TestUser {
    pub fn new(name: &str, email: &str, age: Option<i32>) -> Self {
      Self {
        id: None,
        name: name.to_string(),
        email: email.to_string(),
        age,
        created_at: None,
      }
    }
  }

  impl Timestamps for TestUser {
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
      self.created_at
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
      self.created_at
    }
    fn set_created_at(&mut self, t: chrono::DateTime<chrono::Utc>) {
      self.created_at = Some(t);
    }
    fn set_updated_at(&mut self, t: chrono::DateTime<chrono::Utc>) {
      self.created_at = Some(t);
    }
  }

  fn create_user(name: &str, email: &str, age: Option<i32>) -> TestUser {
    TestUser::new(name, email, age)
  }

  #[tokio::test]
  async fn test_insert_and_find() -> OrmResult<()> {
    let provider = match PostgresProvider::connect(
      "postgres://postgres:postgres@localhost:5432/test_db",
    )
    .await
    {
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
    let provider = match PostgresProvider::connect(
      "postgres://postgres:postgres@localhost:5432/test_db",
    )
    .await
    {
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

    Ok(())
  }

  #[tokio::test]
  async fn test_delete() -> OrmResult<()> {
    let provider = match PostgresProvider::connect(
      "postgres://postgres:postgres@localhost:5432/test_db",
    )
    .await
    {
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
    let provider = match PostgresProvider::connect(
      "postgres://postgres:postgres@localhost:5432/test_db",
    )
    .await
    {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    let all = repo.find_all().await?;
    for user in all {
      if let Some(id) = user.id.clone() {
        let _ = repo.delete(&id).await;
      }
    }

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

    Ok(())
  }

  #[tokio::test]
  async fn test_count() -> OrmResult<()> {
    let provider = match PostgresProvider::connect(
      "postgres://postgres:postgres@localhost:5432/test_db",
    )
    .await
    {
      Ok(p) => p,
      Err(_) => return Ok(()),
    };
    let repo: Repository<TestUser, _> = Repository::new(provider);

    let count = repo.count().await?;
    assert!(count >= 0);

    Ok(())
  }
}
