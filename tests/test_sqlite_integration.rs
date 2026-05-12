#[cfg(test)]
mod sqlite_integration_tests {
  use nosql_orm::error::OrmError;
  use nosql_orm::prelude::*;
  use nosql_orm::providers::sql::SqliteProvider;
  use nosql_orm::relations::WithRelations;
  use nosql_orm::validators::Validate;
  use serde::{Deserialize, Serialize};
  use std::fs;

  fn get_db_path() -> String {
    std::env::var("SQLITE_DB_PATH").unwrap_or_else(|_| "/tmp/test_nosql_orm.db".to_string())
  }

  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct TestNote {
    pub id: Option<String>,
    pub title: String,
    pub content: String,
    pub priority: Option<i32>,
    pub completed: bool,
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

  impl SoftDeletes for TestNote {
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
      None
    }
    fn set_deleted_at(&mut self, _t: Option<chrono::DateTime<chrono::Utc>>) {}
    fn restore(&mut self) {}
    fn is_deleted(&self) -> bool {
      false
    }
  }

  impl TestNote {
    pub fn new(title: &str, content: &str, priority: Option<i32>) -> Self {
      Self {
        id: None,
        title: title.to_string(),
        content: content.to_string(),
        priority,
        completed: false,
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
            "CREATE TABLE test_notes (id TEXT PRIMARY KEY, title TEXT, content TEXT, priority INTEGER, completed INTEGER, tags TEXT, created_at TEXT, updated_at TEXT)",
            vec![],
        ).await?;
    let repo: Repository<TestNote, _> = Repository::new(provider);
    Ok(repo)
  }

  async fn cleanup_notes(repo: &Repository<TestNote, SqliteProvider>) -> OrmResult<()> {
    let all = repo.find_all().await?;
    for note in all {
      if let Some(id) = note.id.clone() {
        let _ = repo.delete(&id).await;
      }
    }
    Ok(())
  }

  #[tokio::test]
  async fn test_connection_and_health_check() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;

    let provider = repo.provider();
    let healthy = provider.health_check().await?;
    assert!(healthy);

    let version = provider.get_server_version().await?;
    assert!(!version.is_empty());

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_insert() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;

    let note = TestNote::new("Test Note", "Hello World", Some(1));
    let saved = repo.save(note).await?;
    assert!(saved.id.is_some());
    assert_eq!(saved.title, "Test Note");

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_find_by_id() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;

    let note = TestNote::new("Find Test", "Finding by ID", Some(2));
    let saved = repo.save(note).await?;

    if let Some(id) = saved.id {
      let found = repo.find_by_id(&id).await?;
      assert!(found.is_some());
      assert_eq!(found.unwrap().content, "Finding by ID");
    }

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_update() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;

    let note = TestNote::new("Original Title", "Original Content", Some(1));
    let saved = repo.save(note).await?;

    let mut updated = saved.clone();
    updated.title = "Updated Title".to_string();
    updated.completed = true;
    let result = repo.save(updated).await?;

    assert_eq!(result.title, "Updated Title");
    assert!(result.completed);

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_delete() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;

    let note = TestNote::new("To Delete", "Will be deleted", Some(1));
    let saved = repo.save(note).await?;

    if let Some(id) = saved.id {
      let deleted = repo.delete(&id).await?;
      assert!(deleted);

      let found = repo.find_by_id(&id).await?;
      assert!(found.is_none());
    }

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_batch_operations() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;

    let notes = vec![
      TestNote::new("Note1", "Content1", Some(1)),
      TestNote::new("Note2", "Content2", Some(2)),
      TestNote::new("Note3", "Content3", Some(3)),
    ];

    let count = repo.insert_many(notes).await?;
    assert_eq!(count, 3);

    let total = repo.count().await?;
    assert!(total >= 3);

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_query_by_priority() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;

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

    let results = repo
      .query()
      .filter(Filter::Gt("priority".to_string(), serde_json::json!(2)))
      .find()
      .await?;
    assert_eq!(results.len(), 3);

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_transaction_commit() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;
    let provider = repo.provider().clone();

    let tx = provider.begin_transaction().await?;
    let note1 = TestNote::new("TxNote1", "TxContent1", Some(1));
    let note2 = TestNote::new("TxNote2", "TxContent2", Some(2));
    repo.save(note1).await?;
    repo.save(note2).await?;
    provider.commit_transaction(tx).await?;

    let all = repo.find_all().await?;
    let tx_notes: Vec<_> = all
      .into_iter()
      .filter(|n| n.title.starts_with("Tx"))
      .collect();
    assert_eq!(tx_notes.len(), 2);

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_transaction_rollback() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;
    let provider = repo.provider().clone();

    let tx = provider.begin_transaction().await?;
    let note1 = TestNote::new("RbNote1", "RbContent1", Some(1));
    let note2 = TestNote::new("RbNote2", "RbContent2", Some(2));
    repo.save(note1).await?;
    repo.save(note2).await?;
    provider.rollback_transaction(tx).await?;

    let all = repo.find_all().await?;
    let rb_notes: Vec<_> = all
      .into_iter()
      .filter(|n| n.title.starts_with("Rb"))
      .collect();
    assert_eq!(rb_notes.len(), 0);

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_update_many() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;

    for i in 1..=3 {
      let note = TestNote::new(
        &format!("UMNote{}", i),
        &format!("UMContent{}", i),
        Some(i as i32),
      );
      repo.save(note).await?;
    }

    let updated = repo
      .update_many(
        Some(Filter::Gt("priority".to_string(), serde_json::json!(1))),
        serde_json::json!({"completed": true}),
      )
      .await?;
    assert_eq!(updated, 2);

    let _ = fs::remove_file(&db_path);

    Ok(())
  }

  #[tokio::test]
  async fn test_delete_many() -> OrmResult<()> {
    let db_path = get_db_path();
    let repo = setup_repo(&db_path).await?;

    for i in 1..=3 {
      let note = TestNote::new(
        &format!("DMNote{}", i),
        &format!("DMContent{}", i),
        Some(i as i32),
      );
      repo.save(note).await?;
    }

    let deleted = repo
      .delete_many(Some(Filter::Eq(
        "priority".to_string(),
        serde_json::json!(2),
      )))
      .await?;
    assert_eq!(deleted, 1);

    let remaining = repo.count().await?;
    assert_eq!(remaining, 2);

    let _ = fs::remove_file(&db_path);

    Ok(())
  }
}
