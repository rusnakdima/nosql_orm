use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{OrmError, OrmResult};
use crate::provider::{AdminCommands, Storage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationMeta {
  pub version: i64,
  pub name: String,
  pub applied_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait Migration<P: Storage>: Send + Sync {
  fn version(&self) -> i64;

  fn name(&self) -> &str;

  async fn up(&self, provider: &P) -> OrmResult<()>;

  async fn down(&self, provider: &P) -> OrmResult<()>;
}

// Runs `up_sql` / `down_sql` through the provider's raw channel.
//
// The `AdminCommands` bound sits on the *impl*, not on `Migration` itself:
// the trait stays `P: Storage` so a custom migration can target a provider
// with no query language, while this type simply does not exist for one
// that cannot run SQL. Every shipped provider implements `AdminCommands`,
// so a provider without a query language answers `NotSupported` at run time
// instead of silently doing nothing — which is what the old no-op did.
pub struct SqlMigration {
  pub version: i64,
  pub name: String,
  pub up_sql: String,
  pub down_sql: String,
}

impl SqlMigration {
  pub fn new(version: i64, name: &str, up_sql: &str, down_sql: &str) -> Self {
    Self {
      version,
      name: name.to_string(),
      up_sql: up_sql.to_string(),
      down_sql: down_sql.to_string(),
    }
  }
}

#[async_trait]
impl<P: Storage + AdminCommands> Migration<P> for SqlMigration {
  fn version(&self) -> i64 {
    self.version
  }

  fn name(&self) -> &str {
    &self.name
  }

  async fn up(&self, provider: &P) -> OrmResult<()> {
    // An empty statement is a legitimate no-op (a one-directional
    // migration), so skip it rather than sending it to be rejected.
    if self.up_sql.trim().is_empty() {
      return Ok(());
    }
    AdminCommands::execute_raw(provider, &self.up_sql, Vec::new()).await?;
    Ok(())
  }

  async fn down(&self, provider: &P) -> OrmResult<()> {
    if self.down_sql.trim().is_empty() {
      return Ok(());
    }
    AdminCommands::execute_raw(provider, &self.down_sql, Vec::new()).await?;
    Ok(())
  }
}

// One step of a `JsonMigration` script.
//
// A script is a JSON array of these, executed in order:
//
// ```json
// [
//   {"op": "create_collection", "collection": "audit"},
//   {"op": "insert", "collection": "settings",
//    "doc": {"id": "app", "theme": "dark"}},
//   {"op": "patch", "collection": "settings", "id": "app",
//    "patch": [{"op": "add", "path": "/email", "value": ""}]},
//   {"op": "update", "collection": "settings", "id": "app",
//    "doc": {"id": "app", "theme": "light"}},
//   {"op": "delete", "collection": "settings", "id": "app"},
//   {"op": "drop_collection", "collection": "audit"}
// ]
// ```
//
// `down_script` is written the same way — the caller states how to undo,
// because only the caller knows what the change was.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum JsonMigrationOp {
  CreateCollection {
    collection: String,
  },
  DropCollection {
    collection: String,
  },
  Insert {
    collection: String,
    doc: Value,
  },
  Update {
    collection: String,
    id: String,
    doc: Value,
  },
  Patch {
    collection: String,
    id: String,
    patch: Vec<JsonPatchOp>,
  },
  Delete {
    collection: String,
    id: String,
  },
}

// A JSON Patch step (RFC 6902 subset: `add`, `remove`, `replace`) used by
// the `patch` op to edit one document in place.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum JsonPatchOp {
  Add { path: String, value: Value },
  Remove { path: String },
  Replace { path: String, value: Value },
}

// RFC 6902 pointer: `/a/b`, with `~1` → `/` and `~0` → `~`.
fn pointer_segments(path: &str) -> OrmResult<Vec<String>> {
  if path.is_empty() || path == "/" {
    return Ok(Vec::new());
  }
  if !path.starts_with('/') {
    return Err(OrmError::InvalidInput(format!(
      "JSON pointer must start with '/': {}",
      path
    )));
  }
  Ok(
    path[1..]
      .split('/')
      .map(|s| s.replace("~1", "/").replace("~0", "~"))
      .collect(),
  )
}

fn apply_patch(doc: &mut Value, ops: &[JsonPatchOp]) -> OrmResult<()> {
  for op in ops {
    match op {
      JsonPatchOp::Add { path, value } => {
        let segments = pointer_segments(path)?;
        set_at(doc, &segments, value.clone(), true)?;
      }
      JsonPatchOp::Replace { path, value } => {
        let segments = pointer_segments(path)?;
        set_at(doc, &segments, value.clone(), false)?;
      }
      JsonPatchOp::Remove { path } => {
        let segments = pointer_segments(path)?;
        remove_at(doc, &segments)?;
      }
    }
  }
  Ok(())
}

// `add` creates missing objects along the way; `replace` requires every
// ancestor to exist, which is what separates the two in RFC 6902.
fn set_at(doc: &mut Value, segments: &[String], value: Value, create: bool) -> OrmResult<()> {
  if segments.is_empty() {
    *doc = value;
    return Ok(());
  }

  let mut current = doc;
  for segment in &segments[..segments.len() - 1] {
    if current.is_null() && create {
      *current = Value::Object(Default::default());
    }
    let obj = current.as_object_mut().ok_or_else(|| {
      OrmError::InvalidInput(format!("cannot descend into non-object at '{}'", segment))
    })?;
    current = obj
      .entry(segment.clone())
      .or_insert_with(|| Value::Object(Default::default()));
  }

  let last = &segments[segments.len() - 1];
  let obj = current
    .as_object_mut()
    .ok_or_else(|| OrmError::InvalidInput(format!("cannot set '{}' on a non-object", last)))?;
  if !create && !obj.contains_key(last) {
    return Err(OrmError::InvalidInput(format!(
      "cannot replace missing field '{}'",
      last
    )));
  }
  obj.insert(last.clone(), value);
  Ok(())
}

fn remove_at(doc: &mut Value, segments: &[String]) -> OrmResult<()> {
  if segments.is_empty() {
    return Err(OrmError::InvalidInput(
      "cannot remove the document root".to_string(),
    ));
  }

  let mut current = doc;
  for segment in &segments[..segments.len() - 1] {
    let obj = current.as_object_mut().ok_or_else(|| {
      OrmError::InvalidInput(format!("cannot descend into non-object at '{}'", segment))
    })?;
    current = obj
      .get_mut(segment)
      .ok_or_else(|| OrmError::NotFound(format!("path segment '{}'", segment)))?;
  }

  let last = &segments[segments.len() - 1];
  let obj = current
    .as_object_mut()
    .ok_or_else(|| OrmError::InvalidInput(format!("cannot remove '{}' from a non-object", last)))?;
  obj
    .remove(last)
    .ok_or_else(|| OrmError::NotFound(format!("path segment '{}'", last)))?;
  Ok(())
}

pub struct JsonMigration {
  pub version: i64,
  pub name: String,
  pub up_script: Value,
  pub down_script: Value,
}

impl JsonMigration {
  pub fn new(version: i64, name: &str, up_script: Value, down_script: Value) -> Self {
    Self {
      version,
      name: name.to_string(),
      up_script,
      down_script,
    }
  }

  async fn run<P: Storage + AdminCommands>(&self, provider: &P, script: &Value) -> OrmResult<()> {
    let ops = script.as_array().ok_or_else(|| {
      OrmError::InvalidInput(
        "a JsonMigration script must be a JSON array of operations".to_string(),
      )
    })?;

    let parsed: Vec<JsonMigrationOp> = ops
      .iter()
      .map(|op| serde_json::from_value(op.clone()).map_err(OrmError::Serialization))
      .collect::<OrmResult<_>>()?;

    for op in parsed {
      match op {
        JsonMigrationOp::CreateCollection { collection } => {
          AdminCommands::create_collection(provider, &collection, None).await?;
        }
        JsonMigrationOp::DropCollection { collection } => {
          AdminCommands::drop_collection(provider, &collection).await?;
        }
        JsonMigrationOp::Insert { collection, doc } => {
          Storage::insert(provider, &collection, doc).await?;
        }
        JsonMigrationOp::Update {
          collection,
          id,
          doc,
        } => {
          Storage::update(provider, &collection, &id, doc).await?;
        }
        JsonMigrationOp::Patch {
          collection,
          id,
          patch,
        } => {
          let mut doc = Storage::find_by_id(provider, &collection, &id)
            .await?
            .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))?;
          apply_patch(&mut doc, &patch)?;
          Storage::update(provider, &collection, &id, doc).await?;
        }
        JsonMigrationOp::Delete { collection, id } => {
          Storage::delete(provider, &collection, &id).await?;
        }
      }
    }
    Ok(())
  }
}

#[async_trait]
impl<P: Storage + AdminCommands> Migration<P> for JsonMigration {
  fn version(&self) -> i64 {
    self.version
  }

  fn name(&self) -> &str {
    &self.name
  }

  async fn up(&self, provider: &P) -> OrmResult<()> {
    self.run(provider, &self.up_script).await
  }

  async fn down(&self, provider: &P) -> OrmResult<()> {
    self.run(provider, &self.down_script).await
  }
}
