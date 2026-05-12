use crate::cdc::{Change, ChangeType};
use crate::error::OrmResult;
use chrono::{DateTime, Utc};

pub struct ChangeStream {
  changes: Vec<Change>,
}

impl ChangeStream {
  pub fn new(changes: Vec<Change>) -> Self {
    Self { changes }
  }

  #[cfg(feature = "mongo")]
  pub async fn from_mongo_stream<T: serde::de::DeserializeOwned + serde::Serialize + Unpin + Send + Sync>(
    stream: impl futures::Stream<
      Item = Result<mongodb::change_stream::event::ChangeStreamEvent<T>, mongodb::error::Error>,
    > + Unpin,
  ) -> OrmResult<Self> {
    use futures::StreamExt;
    let mut changes = Vec::new();
    let mut stream = stream;
    while let Some(result) = stream.next().await {
      match result {
        Ok(event) => {
          let change = Self::convert_mongo_event(event)?;
          changes.push(change);
        }
        Err(e) => {
          tracing::warn!("Error reading change stream event: {:?}", e);
        }
      }
    }
    Ok(Self::new(changes))
  }

  #[cfg(not(feature = "mongo"))]
  pub async fn from_mongo_stream<T>(_stream: impl Send + Sync) -> OrmResult<Self>
  where
    T: serde::de::DeserializeOwned + Unpin,
  {
    Err(crate::error::OrmError::NotSupported(
      "MongoDB support not enabled".to_string(),
    ))
  }

  #[cfg(feature = "mongo")]
  fn convert_mongo_event<T: serde::de::DeserializeOwned + serde::Serialize>(
    event: mongodb::change_stream::event::ChangeStreamEvent<T>,
  ) -> OrmResult<Change> {
    let change_type = match event.operation_type {
      mongodb::change_stream::event::OperationType::Insert => ChangeType::Insert,
      mongodb::change_stream::event::OperationType::Update => ChangeType::Update,
      mongodb::change_stream::event::OperationType::Replace => ChangeType::Update,
      mongodb::change_stream::event::OperationType::Delete => ChangeType::Delete,
      _ => ChangeType::Update,
    };

    let collection = event.ns.and_then(|ns| ns.coll).unwrap_or_default();

    let entity_id = event
      .document_key
      .and_then(|dk| dk.get("_id").and_then(|id| id.as_str().map(String::from)))
      .unwrap_or_default();

    let before = event.full_document_before_change.as_ref().map(|d| serde_json::to_value(d).ok()).flatten();
    let after = event.full_document.as_ref().map(|d| serde_json::to_value(d).ok()).flatten();

    let timestamp = event
      .cluster_time
      .map(|ct| DateTime::from_timestamp(ct.time as i64, 0).unwrap_or_else(Utc::now))
      .unwrap_or_else(Utc::now);

    let user_id = None;

    Ok(Change {
      id: uuid::Uuid::new_v4().to_string(),
      change_type,
      collection,
      entity_id,
      before,
      after,
      timestamp,
      user_id,
      trace_id: None,
    })
  }

  pub fn filter_collection(mut self, collection: &str) -> Self {
    self.changes.retain(|c| c.collection == collection);
    self
  }

  pub fn filter_type(mut self, change_type: ChangeType) -> Self {
    self.changes.retain(|c| c.change_type == change_type);
    self
  }

  pub fn into_vec(self) -> Vec<Change> {
    self.changes
  }
}
