use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub trait SoftDeletable: Send + Sync {
  fn deleted_at(&self) -> Option<DateTime<Utc>>;
  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>);
  fn is_deleted(&self) -> bool {
    self.deleted_at().is_some()
  }
  fn mark_deleted(&mut self) {
    self.set_deleted_at(Some(Utc::now()));
  }
  fn restore(&mut self) {
    self.set_deleted_at(None);
  }
}

impl SoftDeletable for Option<DateTime<Utc>> {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    *self
  }
  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    *self = deleted_at;
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftDeleteExt {
  pub deleted_at: Option<DateTime<Utc>>,
}

impl SoftDeletable for SoftDeleteExt {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    self.deleted_at = deleted_at;
  }
}
