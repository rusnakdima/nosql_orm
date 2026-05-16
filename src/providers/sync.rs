use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub enum ConflictResolution {
  PreferSource,
  PreferTarget,
  LatestWins,
}

#[derive(Debug, Clone)]
pub struct SyncOptions {
  pub update_timestamps: bool,
  pub conflict_resolution: ConflictResolution,
}

impl Default for SyncOptions {
  fn default() -> Self {
    Self {
      update_timestamps: false,
      conflict_resolution: ConflictResolution::PreferSource,
    }
  }
}

pub struct ProviderSync;

impl ProviderSync {
  pub async fn sync_entity<S, T>(
    source: &S,
    target: &T,
    collection: &str,
    id: &str,
    options: SyncOptions,
  ) -> OrmResult<bool>
  where
    S: DatabaseProvider,
    T: DatabaseProvider,
  {
    let entity = source.find_by_id(collection, id).await?;
    match entity {
      Some(mut doc) => {
        if options.update_timestamps {
          if let Some(obj) = doc.as_object_mut() {
            obj.insert(
              "updated_at".to_string(),
              serde_json::json!(chrono::Utc::now().to_rfc3339()),
            );
          }
        }

        let sanitized = sanitize_for_target(doc);

        match target.find_by_id(collection, id).await? {
          Some(_) => {
            target.update(collection, id, sanitized).await?;
          }
          None => {
            target.insert(collection, sanitized).await?;
          }
        }
        Ok(true)
      }
      None => Ok(false),
    }
  }

  pub async fn sync_collection<S, T>(
    source: &S,
    target: &T,
    collection: &str,
    filter: &crate::query::Filter,
    options: SyncOptions,
  ) -> OrmResult<usize>
  where
    S: DatabaseProvider,
    T: DatabaseProvider,
  {
    let entities = source
      .find_many(collection, Some(filter), None, None, None, true)
      .await?;
    let mut count = 0;
    for entity in entities {
      if let Some(id) = entity.get("id").and_then(|v| v.as_str()) {
        if Self::sync_entity(source, target, collection, id, options.clone()).await? {
          count += 1;
        }
      }
    }
    Ok(count)
  }
}

fn sanitize_for_target(mut doc: Value) -> Value {
  if let serde_json::Value::Object(ref obj) = doc {
    let mut filtered = serde_json::Map::new();
    for (k, v) in obj.iter() {
      if !k.starts_with('$') {
        filtered.insert(k.clone(), sanitize_for_target(v.clone()));
      }
    }
    serde_json::Value::Object(filtered)
  } else {
    doc
  }
}
