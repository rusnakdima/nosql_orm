use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use chrono::DateTime;
use serde_json::Value;

pub struct CdcSync<P: DatabaseProvider> {
  provider: P,
}

impl<P: DatabaseProvider> CdcSync<P> {
  pub fn new(provider: P) -> Self {
    Self { provider }
  }

  pub fn provider(&self) -> &P {
    &self.provider
  }

  pub async fn sync_to(
    &self,
    source: &P,
    target: &P,
    collection: &str,
  ) -> OrmResult<(usize, usize)> {
    let source_records = source.find_all(collection).await?;

    let mut synced_count = 0;
    let mut skipped_count = 0;

    for source_record in source_records {
      let source_id = source_record
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| OrmError::InvalidInput("Record missing id field".to_string()))?
        .to_string();

      let target_record = target.find_by_id(collection, &source_id).await?;

      if self.should_sync(&source_record, &target_record) {
        target.update(collection, &source_id, source_record).await?;
        synced_count += 1;
      } else {
        skipped_count += 1;
      }
    }

    Ok((synced_count, skipped_count))
  }

  pub async fn sync_to_default(&self, source: &P, collection: &str) -> OrmResult<(usize, usize)> {
    self.sync_to(source, &self.provider, collection).await
  }

  fn should_sync(&self, source: &Value, target: &Option<Value>) -> bool {
    let source_updated = match source.get("updated_at").and_then(|v| v.as_str()) {
      Some(s) => s,
      None => return true,
    };

    match target {
      Some(t) => {
        let target_updated = match t.get("updated_at").and_then(|v| v.as_str()) {
          Some(s) => s,
          None => return true,
        };

        let source_ts = match DateTime::parse_from_rfc3339(source_updated) {
          Ok(ts) => ts,
          Err(_) => return true,
        };

        let target_ts = match DateTime::parse_from_rfc3339(target_updated) {
          Ok(ts) => ts,
          Err(_) => return true,
        };

        source_ts > target_ts
      }
      None => true,
    }
  }
}
