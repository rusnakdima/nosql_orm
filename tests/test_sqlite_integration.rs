#[cfg(test)]
mod sqlite_integration_tests {
  use nosql_orm::prelude::*;
  use nosql_orm::providers::sql::SqliteProvider;
  use nosql_orm::relations::WithRelations;
  use nosql_orm::validators::Validate;
  use serde::{Deserialize, Serialize};
  use std::fs;

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct TestNote {
    pub id: Option<String>,
    pub title: String,
    pub content: String,
    pub priority: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
  }

  impl Entity for TestNote {
    fn meta() -> EntityMeta {
      EntityMeta::new("test_notes")
    }
    fn get_id(&self) -> Option<String> {
      self.id.clone()
    }
    fn set_id(&mut self, id: String) {
      self.id = Some(id);
    }
  }

  impl WithRelations for TestNote {
    fn relations() -> Vec<nosql_orm::relations::RelationDef> {
      vec![]
    }
  }

  impl Validate for TestNote {
    fn validate(&self) -> OrmResult<()> {
      Ok(())
    }
  }

  impl Timestamps for TestNote {
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

  impl TestNote {
    pub fn new(title: &str, content: &str, priority: Option<i32>) -> Self {
      Self {
        id: None,
        title: title.to_string(),
        content: content.to_string(),
        priority,
        tags: None,
        created_at: None,
        updated_at: None,
      }
    }
  }

  async fn setup_repo(db_path: &str) -> OrmResult<Repository<TestNote, SqliteProvider>> {
    let _ = fs::remove_file(db_path);

    let provider = SqliteProvider::connect(db_path).await?;
    provider.execute_raw(
            "CREATE TABLE test_notes (id TEXT PRIMARY KEY, title TEXT, content TEXT, priority INTEGER, tags TEXT, created_at TEXT, updated_at TEXT)",
            vec![],
        ).await?;
    let repo: Repository<TestNote, _> = Repository::new(provider);
    Ok(repo)
  }

  #[tokio::test]
  async fn test_sqlite_insert() -> OrmResult<()> {
    let repo = setup_repo("/tmp/test_nosql_orm_sqlite_insert.db").await?;

    let note = TestNote::new("Test Note", "Hello World", Some(1));
    let saved = repo.save(note).await?;
    assert!(saved.id.is_some());
    assert_eq!(saved.title, "Test Note");

    let _ = fs::remove_file("/tmp/test_nosql_orm_sqlite_insert.db");

    Ok(())
  }

  #[tokio::test]
  async fn test_sqlite_batch_operations() -> OrmResult<()> {
    let repo = setup_repo("/tmp/test_nosql_orm_sqlite_batch.db").await?;

    let notes = vec![
      TestNote::new("Note1", "Content1", Some(1)),
      TestNote::new("Note2", "Content2", Some(2)),
      TestNote::new("Note3", "Content3", Some(3)),
    ];

    for note in notes {
      repo.save(note).await?;
    }

    let count = repo.count().await?;
    assert!(count >= 3);

    let _ = fs::remove_file("/tmp/test_nosql_orm_sqlite_batch.db");

    Ok(())
  }

  #[tokio::test]
  async fn test_sqlite_query_by_priority() -> OrmResult<()> {
    let repo = setup_repo("/tmp/test_nosql_orm_sqlite_qb.db").await?;

    for i in 1..=5 {
      let note = TestNote::new(
        &format!("Note{}", i),
        &format!("Content{}", i),
        Some(i as i32),
      );
      repo.save(note).await?;
    }

    let count = repo.count().await?;
    assert_eq!(count, 5);

    let _ = fs::remove_file("/tmp/test_nosql_orm_sqlite_qb.db");

    Ok(())
  }
}
