use crate::error::{OrmError, OrmResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
  Insert,
  Update,
  Delete,
  SoftDelete,
  Restore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
  pub id: String,
  pub change_type: ChangeType,
  pub collection: String,
  pub entity_id: String,
  pub before: Option<serde_json::Value>,
  pub after: Option<serde_json::Value>,
  pub timestamp: DateTime<Utc>,
  pub user_id: Option<String>,
  pub trace_id: Option<String>,
}

impl Change {
  pub fn insert(collection: &str, entity_id: &str, data: serde_json::Value) -> Self {
    Self {
      id: uuid::Uuid::new_v4().to_string(),
      change_type: ChangeType::Insert,
      collection: collection.to_string(),
      entity_id: entity_id.to_string(),
      before: None,
      after: Some(data),
      timestamp: Utc::now(),
      user_id: None,
      trace_id: None,
    }
  }

  pub fn update(
    collection: &str,
    entity_id: &str,
    before: serde_json::Value,
    after: serde_json::Value,
  ) -> Self {
    Self {
      id: uuid::Uuid::new_v4().to_string(),
      change_type: ChangeType::Update,
      collection: collection.to_string(),
      entity_id: entity_id.to_string(),
      before: Some(before),
      after: Some(after),
      timestamp: Utc::now(),
      user_id: None,
      trace_id: None,
    }
  }

  pub fn delete(collection: &str, entity_id: &str, data: serde_json::Value) -> Self {
    Self {
      id: uuid::Uuid::new_v4().to_string(),
      change_type: ChangeType::Delete,
      collection: collection.to_string(),
      entity_id: entity_id.to_string(),
      before: Some(data),
      after: None,
      timestamp: Utc::now(),
      user_id: None,
      trace_id: None,
    }
  }
}

#[async_trait::async_trait]
pub trait ChangeCapture: Send + Sync {
  async fn capture(&self, change: Change) -> OrmResult<()>;
  async fn get_changes(
    &self,
    collection: &str,
    since: chrono::DateTime<Utc>,
  ) -> OrmResult<Vec<Change>>;
  async fn get_entity_history(&self, collection: &str, entity_id: &str) -> OrmResult<Vec<Change>>;
}

pub struct JsonChangeCapture {
  storage_path: std::path::PathBuf,
}

impl JsonChangeCapture {
  pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
    Self {
      storage_path: path.into(),
    }
  }

  fn changes_dir(&self) -> std::path::PathBuf {
    self.storage_path.join("changes")
  }

  fn validate_collection_name(name: &str) -> OrmResult<()> {
    if name.is_empty() {
      return Err(OrmError::InvalidInput(
        "Collection name cannot be empty".to_string(),
      ));
    }
    if name.len() > 255 {
      return Err(OrmError::InvalidInput(
        "Collection name exceeds maximum length of 255".to_string(),
      ));
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') {
      return Err(OrmError::InvalidInput(
        "Collection name contains invalid characters (path traversal)".to_string(),
      ));
    }
    if name.starts_with('.') {
      return Err(OrmError::InvalidInput(
        "Collection name cannot start with a dot".to_string(),
      ));
    }
    Ok(())
  }

  fn change_file(&self, collection: &str) -> OrmResult<std::path::PathBuf> {
    Self::validate_collection_name(collection)?;
    Ok(self.changes_dir().join(format!("{}.json", collection)))
  }

  async fn ensure_dir(&self) -> OrmResult<()> {
    tokio::fs::create_dir_all(self.changes_dir()).await?;
    Ok(())
  }

  async fn read_changes(&self, collection: &str) -> OrmResult<Vec<Change>> {
    let path = self.change_file(collection)?;
    if !path.exists() {
      return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(&path).await?;
    if content.is_empty() {
      return Ok(Vec::new());
    }
    let changes: Vec<Change> = serde_json::from_str(&content)?;
    Ok(changes)
  }

  async fn write_changes(&self, collection: &str, changes: &[Change]) -> OrmResult<()> {
    self.ensure_dir().await?;
    let path = self.change_file(collection)?;
    let content = serde_json::to_string_pretty(changes)?;
    tokio::fs::write(&path, content).await?;
    Ok(())
  }
}

#[async_trait::async_trait]
impl ChangeCapture for JsonChangeCapture {
  async fn capture(&self, change: Change) -> OrmResult<()> {
    let collection = change.collection.clone();
    let mut changes = self.read_changes(&collection).await?;
    changes.push(change);
    self.write_changes(&collection, &changes).await?;
    Ok(())
  }

  async fn get_changes(
    &self,
    collection: &str,
    since: chrono::DateTime<Utc>,
  ) -> OrmResult<Vec<Change>> {
    let changes = self.read_changes(collection).await?;
    Ok(
      changes
        .into_iter()
        .filter(|c| c.timestamp >= since)
        .collect(),
    )
  }

  async fn get_entity_history(&self, collection: &str, entity_id: &str) -> OrmResult<Vec<Change>> {
    let changes = self.read_changes(collection).await?;
    Ok(
      changes
        .into_iter()
        .filter(|c| c.entity_id == entity_id)
        .collect(),
    )
  }
}

#[cfg(feature = "mongo")]
pub struct MongoChangeCapture {
  collection: mongodb::Collection<Change>,
}

#[cfg(feature = "mongo")]
impl MongoChangeCapture {
  pub fn new(collection: mongodb::Collection<Change>) -> Self {
    Self { collection }
  }
}

#[cfg(feature = "mongo")]
#[async_trait::async_trait]
impl ChangeCapture for MongoChangeCapture {
  async fn capture(&self, change: Change) -> OrmResult<()> {
    self.collection.insert_one(change, None).await?;
    Ok(())
  }

  async fn get_changes(
    &self,
    collection: &str,
    since: chrono::DateTime<Utc>,
  ) -> OrmResult<Vec<Change>> {
    use futures::TryStreamExt;
    use mongodb::bson::doc;
    let sys_time = std::time::SystemTime::from(since);
    let bson_dt = mongodb::bson::DateTime::from(sys_time);
    let filter = doc! {
      "collection": collection,
      "timestamp": { "$gte": bson_dt }
    };
    let mut cursor = self.collection.find(filter, None).await?;
    let mut changes = Vec::new();
    while let Some(result) = cursor.try_next().await? {
      changes.push(result);
    }
    Ok(changes)
  }

  async fn get_entity_history(&self, collection: &str, entity_id: &str) -> OrmResult<Vec<Change>> {
    use futures::TryStreamExt;
    use mongodb::bson::doc;
    let filter = doc! {
      "collection": collection,
      "entity_id": entity_id
    };
    let mut cursor = self.collection.find(filter, None).await?;
    let mut changes = Vec::new();
    while let Some(result) = cursor.try_next().await? {
      changes.push(result);
    }
    Ok(changes)
  }
}
