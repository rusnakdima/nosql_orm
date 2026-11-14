// Offline-first sync: queue writes while offline, sync when connected.
//
// Wraps a local and remote Storage. Writes go to local immediately and are
// queued for sync. `sync()` drains the queue and applies to remote.

use crate::error::{OrmError, OrmResult};
use crate::provider::Storage;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum SyncStatus {
  Pending,
  Syncing,
  Synced,
  Conflict(String),
  Failed(String),
}

#[derive(Debug, Clone)]
pub struct SyncEntry {
  pub collection: String,
  pub op: SyncOp,
  pub doc: Value,
  pub status: SyncStatus,
}

#[derive(Debug, Clone)]
pub enum SyncOp {
  Insert,
  Update,
  Delete,
}

#[derive(Clone)]
pub struct SyncEngine<L: Storage, R: Storage> {
  local: L,
  remote: R,
  queue: Arc<Mutex<VecDeque<SyncEntry>>>,
}

impl<L: Storage, R: Storage> SyncEngine<L, R> {
  pub fn new(local: L, remote: R) -> Self {
    Self {
      local,
      remote,
      queue: Arc::new(Mutex::new(VecDeque::new())),
    }
  }

  // Queue a write for sync (writes to local immediately).
  pub async fn enqueue_write(
    &self,
    collection: &str,
    op: SyncOp,
    doc: Value,
  ) -> OrmResult<()> {
    // Write to local immediately
    match op {
      SyncOp::Insert => {
        self.local.insert(collection, doc.clone()).await?;
      }
      SyncOp::Update => {
        if let Some(id) = doc.get("id").and_then(Value::as_str) {
          self.local.update(collection, id, doc.clone()).await?;
        }
      }
      SyncOp::Delete => {
        if let Some(id) = doc.get("id").and_then(Value::as_str) {
          self.local.delete(collection, id).await?;
        }
      }
    }

    // Queue for sync
    let mut queue = self.queue.lock().expect("lock poisoned");
    queue.push_back(SyncEntry {
      collection: collection.to_string(),
      op,
      doc,
      status: SyncStatus::Pending,
    });
    Ok(())
  }

  // Sync all pending entries to remote.
  pub async fn sync(&self) -> OrmResult<usize> {
    let entries: Vec<SyncEntry> = {
      let mut queue = self.queue.lock().expect("lock poisoned");
      queue.drain(..).collect()
    };

    let mut synced = 0;
    let mut failed_entries = Vec::new();

    for mut entry in entries {
      entry.status = SyncStatus::Syncing;
      let result = match entry.op {
        SyncOp::Insert => self.remote.insert(&entry.collection, entry.doc.clone()).await.map(|_| ()),
        SyncOp::Update => {
          if let Some(id) = entry.doc.get("id").and_then(Value::as_str) {
            self.remote.update(&entry.collection, id, entry.doc.clone()).await.map(|_| ())
          } else {
            Err(OrmError::InvalidInput("missing id".to_string()))
          }
        }
        SyncOp::Delete => {
          if let Some(id) = entry.doc.get("id").and_then(Value::as_str) {
            self.remote.delete(&entry.collection, id).await.map(|_| ())
          } else {
            Err(OrmError::InvalidInput("missing id".to_string()))
          }
        }
      };

      match result {
        Ok(_) => {
          entry.status = SyncStatus::Synced;
          synced += 1;
        }
        Err(e) => {
          entry.status = SyncStatus::Failed(e.to_string());
          failed_entries.push(entry);
        }
      }
    }

    // Re-queue failed entries
    if !failed_entries.is_empty() {
      let mut queue = self.queue.lock().expect("lock poisoned");
      for entry in failed_entries {
        queue.push_back(entry);
      }
    }

    Ok(synced)
  }

  // Number of pending entries.
  pub fn pending_count(&self) -> usize {
    self.queue.lock().expect("lock poisoned").len()
  }
}