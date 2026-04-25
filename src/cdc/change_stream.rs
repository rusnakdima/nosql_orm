use crate::cdc::Change;
use crate::error::OrmResult;

pub struct ChangeStream {
  changes: Vec<Change>,
}

impl ChangeStream {
  pub fn new(changes: Vec<Change>) -> Self {
    Self { changes }
  }

  #[cfg(feature = "mongo")]
  pub async fn from_mongo_stream<T: serde::de::DeserializeOwned + Unpin>(
    _stream: mongodb::change_stream::ChangeStream<T>,
  ) -> OrmResult<Self> {
    let mut changes = Vec::new();
    Ok(Self { changes })
  }

  pub fn filter_collection(mut self, collection: &str) -> Self {
    self.changes.retain(|c| c.collection == collection);
    self
  }

  pub fn filter_type(mut self, change_type: crate::cdc::ChangeType) -> Self {
    self.changes.retain(|c| c.change_type == change_type);
    self
  }

  pub fn into_vec(self) -> Vec<Change> {
    self.changes
  }
}
