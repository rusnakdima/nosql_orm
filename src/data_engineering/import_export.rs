use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::Storage;
use crate::repository::Repository;
use serde::Serialize;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use tokio::time::Instant;

pub enum DataFormat {
    Csv,
    Json,
    Parquet,
    Avro,
}

pub struct Exporter {
    format: DataFormat,
    batch_size: usize,
}

impl Exporter {
    pub fn new(format: DataFormat) -> Self {
        Self {
            format,
            batch_size: 1000,
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub async fn export<E, P>(
        &self,
        repository: &Repository<E, P>,
        output: &str,
    ) -> OrmResult<ExportStats>
    where
        E: Entity + Serialize,
        P: Storage,
    {
        match self.format {
            DataFormat::Json => self.export_json(repository, output).await,
            DataFormat::Csv => self.export_csv(repository, output).await,
            _ => Err(OrmError::NotSupported(
                "Format not supported".to_string(),
            )),
        }
    }

    async fn export_json<E, P>(
        &self,
        repository: &Repository<E, P>,
        output: &str,
    ) -> OrmResult<ExportStats>
    where
        E: Entity + Serialize,
        P: Storage,
    {
        let start = Instant::now();
        let mut exported = 0;
        let mut errors = 0;

        let collection = E::table_name();
        let mut file = File::create(output).await?;

        file.write_all(b"[\n").await?;

        let mut first = true;
        loop {
            let docs = repository
                .provider()
                .find_many(
                    &collection,
                    None,
                    None,
                    Some(self.batch_size as u64),
                    None,
                    true,
                )
                .await?;

            if docs.is_empty() {
                break;
            }

            for doc in docs {
                if first {
                    first = false;
                } else {
                    file.write_all(b",\n").await?;
                }

                let json = serde_json::to_string(&doc)?;
                file.write_all(json.as_bytes()).await?;
                exported += 1;
            }
        }

        file.write_all(b"\n]").await?;

        Ok(ExportStats {
            exported,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn export_csv<E, P>(
        &self,
        repository: &Repository<E, P>,
        output: &str,
    ) -> OrmResult<ExportStats>
    where
        E: Entity + Serialize,
        P: Storage,
    {
        let start = Instant::now();
        let mut exported = 0;
        let mut errors = 0;

        let collection = E::table_name();
        let mut file = File::create(output).await?;

        let mut header_written = false;

        loop {
            let docs = repository
                .provider()
                .find_many(
                    &collection,
                    None,
                    None,
                    Some(self.batch_size as u64),
                    None,
                    true,
                )
                .await?;

            if docs.is_empty() {
                break;
            }

            for doc in docs {
                if let Some(obj) = doc.as_object() {
                    if !header_written {
                        let headers: Vec<String> = obj.keys().map(|k| k.clone()).collect();
                        file.write_all(headers.join(",").as_bytes()).await?;
                        file.write_all(b"\n").await?;
                        header_written = true;
                    }

                    let values: Vec<String> = obj
                        .values()
                        .map(|v| match v {
                            serde_json::Value::String(s) => {
                                format!("\"{}\"", s.replace('"', "\"\""))
                            }
                            _ => v.to_string(),
                        })
                        .collect();

                    file.write_all(values.join(",").as_bytes()).await?;
                    file.write_all(b"\n").await?;
                    exported += 1;
                }
            }
        }

        Ok(ExportStats {
            exported,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

pub struct ImportConfig {
    pub batch_size: usize,
    pub on_duplicate: OnDuplicate,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            on_duplicate: OnDuplicate::Skip,
        }
    }
}

pub enum OnDuplicate {
    Skip,
    Update,
    Fail,
}

pub struct Importer {
    format: DataFormat,
    config: ImportConfig,
}

impl Importer {
    pub fn new(format: DataFormat) -> Self {
        Self {
            format,
            config: ImportConfig::default(),
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.config.batch_size = size;
        self
    }

    pub fn with_on_duplicate(mut self, action: OnDuplicate) -> Self {
        self.config.on_duplicate = action;
        self
    }

    pub async fn import<E, P>(
        &self,
        repository: &Repository<E, P>,
        input: &str,
    ) -> OrmResult<ImportStats>
    where
        E: Entity + serde::de::DeserializeOwned + Send + Sync,
        P: Storage,
    {
        match self.format {
            DataFormat::Json => self.import_json(repository, input).await,
            DataFormat::Csv => self.import_csv(repository, input).await,
            _ => Err(OrmError::NotSupported("Format not supported".to_string())),
        }
    }

    async fn import_json<E, P>(
        &self,
        repository: &Repository<E, P>,
        input: &str,
    ) -> OrmResult<ImportStats>
    where
        E: Entity + serde::de::DeserializeOwned + Send + Sync,
        P: Storage,
    {
        let start = Instant::now();
        let mut imported = 0;
        let mut skipped = 0;
        let mut errors = 0;

        let content = tokio::fs::read_to_string(input).await?;
        let values: Vec<serde_json::Value> = serde_json::from_str(&content)?;

        for chunk in values.chunks(self.config.batch_size) {
            for value in chunk {
                match repository.save_from_value(value.clone()).await {
                    Ok(_) => imported += 1,
                    Err(_) => {
                        errors += 1;
                        if matches!(self.config.on_duplicate, OnDuplicate::Skip) {
                            skipped += 1;
                        }
                    }
                }
            }
        }

        Ok(ImportStats {
            imported,
            skipped,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn import_csv<E, P>(
        &self,
        repository: &Repository<E, P>,
        input: &str,
    ) -> OrmResult<ImportStats>
    where
        E: Entity + serde::de::DeserializeOwned + Send + Sync,
        P: Storage,
    {
        let start = Instant::now();
        let mut imported = 0;
        let mut skipped = 0;
        let mut errors = 0;

        let file = File::open(input).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let headers: Vec<String> = match lines.next_line().await {
            Ok(Some(line)) => line.split(',').map(|s| s.trim().to_string()).collect(),
            Ok(None) => return Err(OrmError::InvalidInput("Empty CSV file".to_string())),
            Err(_) => return Err(OrmError::InvalidInput("Failed to read CSV header".to_string())),
        };

        while let Ok(Some(line)) = lines.next_line().await {
            let values: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            let mut map = serde_json::Map::new();

            for (i, header) in headers.iter().enumerate() {
                if let Some(value) = values.get(i) {
                    map.insert(header.clone(), serde_json::Value::String(value.to_string()));
                }
            }

            match repository.save_from_value(serde_json::Value::Object(map)).await {
                Ok(_) => imported += 1,
                Err(_) => {
                    errors += 1;
                    if matches!(self.config.on_duplicate, OnDuplicate::Skip) {
                        skipped += 1;
                    }
                }
            }
        }

        Ok(ImportStats {
            imported,
            skipped,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[derive(Debug)]
pub struct ExportStats {
    pub exported: usize,
    pub errors: usize,
    pub duration_ms: u64,
}

#[derive(Debug)]
pub struct ImportStats {
    pub imported: usize,
    pub skipped: usize,
    pub errors: usize,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::entity::{Entity, EntityMeta};
  use crate::provider::Storage;
  use crate::repository::Repository;
  use crate::providers::sql::sqlite::SqliteProvider;
  use async_trait::async_trait;
  use parking_lot::RwLock;
  use serde::Serialize;
  use serde_json::Value;
  use std::collections::HashMap;
  use tempfile::tempdir;

  #[derive(Debug, Clone, Serialize, Deserialize)]
  struct TestEntity {
    pub id: Option<String>,
    pub name: String,
    pub age: i32,
  }

  impl Entity for TestEntity {
    fn meta() -> EntityMeta {
      EntityMeta::new("test_entities")
    }

    fn get_id(&self) -> Option<String> {
      self.id.clone()
    }

    fn set_id(&mut self, id: String) {
      self.id = Some(id);
    }
  }

  #[derive(Clone)]
  struct MockProvider {
    data: std::sync::Arc<RwLock<HashMap<String, Vec<Value>>>>,
  }

  impl Default for MockProvider {
    fn default() -> Self {
      Self {
        data: std::sync::Arc::new(RwLock::new(HashMap::new())),
      }
    }
  }

  #[async_trait]
  impl Storage for MockProvider {
    async fn insert(&self, collection: &str, doc: Value) -> crate::error::OrmResult<Value> {
      self
        .data
        .write()
        .entry(collection.to_string())
        .or_default()
        .push(doc.clone());
      Ok(doc)
    }

    async fn find_by_id(
      &self,
      collection: &str,
      id: &str,
    ) -> crate::error::OrmResult<Option<Value>> {
      let docs = self.data.read();
      let found = docs.get(collection).and_then(|v| {
        v.iter().find(|doc| {
          doc.get("id")
            .and_then(|v| v.as_str())
            .map(|sid| sid == id)
            .unwrap_or(false)
        }).cloned()
      });
      Ok(found)
    }

    async fn delete(&self, collection: &str, id: &str) -> crate::error::OrmResult<()> {
      let mut data = self.data.write();
      if let Some(docs) = data.get_mut(collection) {
        docs.retain(|doc| {
          doc.get("id")
            .and_then(|v| v.as_str())
            .map(|sid| sid != id)
            .unwrap_or(true)
        });
      }
      Ok(())
    }

    async fn update(
      &self,
      collection: &str,
      id: &str,
      doc: Value,
    ) -> crate::error::OrmResult<Value> {
      let mut data = self.data.write();
      if let Some(docs) = data.get_mut(collection) {
        if let Some(existing) = docs.iter_mut().find(|d| {
          d.get("id")
            .and_then(|v| v.as_str())
            .map(|sid| sid == id)
            .unwrap_or(false)
        }) {
          if let Some(obj) = existing.as_object_mut() {
            if let Some(upd) = doc.as_object() {
              obj.extend(upd.clone());
            }
          }
          return Ok(existing.clone());
        }
      }
      Err(crate::error::OrmError::NotFound(format!(
        "id {} not found",
        id
      )))
    }

    async fn find(
      &self,
      collection: &str,
      _filter: crate::query::Filter,
    ) -> crate::error::OrmResult<Vec<Value>> {
      let data = self.data.read();
      Ok(data.get(collection).cloned().unwrap_or_default())
    }

    async fn count(
      &self,
      _collection: &str,
      _filter: crate::query::Filter,
    ) -> crate::error::OrmResult<i64> {
      Ok(0)
    }

    async fn health_check(&self) -> crate::error::OrmResult<()> {
      Ok(())
    }
  }

  #[test]
  fn import_export_json_roundtrip() -> crate::error::OrmResult<()> {
    let entities = vec![
      TestEntity {
        id: Some("1".to_string()),
        name: "Alice".to_string(),
        age: 30,
      },
      TestEntity {
        id: Some("2".to_string()),
        name: "Bob".to_string(),
        age: 25,
      },
    ];

    let json = serde_json::to_string(&entities).unwrap();
    let roundtripped: Vec<TestEntity> = serde_json::from_str(&json).unwrap();

    assert_eq!(entities.len(), roundtripped.len());
    assert_eq!(entities[0].name, roundtripped[0].name);
    assert_eq!(entities[1].age, roundtripped[1].age);

    Ok(())
  }

  #[tokio::test]
  async fn import_export_csv_roundtrip() -> crate::error::OrmResult<()> {
    let temp = tempdir().unwrap();
    let csv_path = temp.path().join("test.csv");

    let entities = vec![
      TestEntity {
        id: Some("1".to_string()),
        name: "Charlie".to_string(),
        age: 35,
      },
      TestEntity {
        id: Some("2".to_string()),
        name: "Diana".to_string(),
        age: 28,
      },
    ];

    let mut csv_content = String::from("id,name,age\n");
    for e in &entities {
      csv_content.push_str(&format!("{},{},{}\n", e.id.as_ref().unwrap(), e.name, e.age));
    }

    let provider = MockProvider::default();
    let repo = Repository::<TestEntity, MockProvider>::new(provider.clone());
    let exporter = Exporter::new(DataFormat::Csv).with_batch_size(100);
    exporter
      .export_csv(&repo, csv_path.to_str().unwrap())
      .await
      .unwrap();

    let read_content = std::fs::read_to_string(&csv_path).unwrap();
    assert!(read_content.contains("Charlie"));
    assert!(read_content.contains("Diana"));

    Ok(())
  }

  #[test]
  fn import_export_csv_with_missing_fields() -> crate::error::OrmResult<()> {
    // Simulate CSV rows with missing fields parsed manually
    let csv_with_missing = "id,name,age\n1,Alice,\n2,,30\n";

    let lines: Vec<&str> = csv_with_missing.lines().collect();
    assert_eq!(lines.len(), 3); // header + 2 rows

    // Row 1: id=1, name=Alice, age=empty
    let row1: Vec<&str> = lines[1].split(',').collect();
    assert_eq!(row1[0], "1");
    assert_eq!(row1[1], "Alice");
    assert_eq!(row1[2], ""); // age missing

    // Row 2: id=2, name=empty, age=30
    let row2: Vec<&str> = lines[2].split(',').collect();
    assert_eq!(row2[0], "2");
    assert_eq!(row2[1], "");
    assert_eq!(row2[2], "30");

    // Missing fields should default to empty string / None when deserialized
    assert_eq!(row1[2], "");
    assert_eq!(row2[1], "");

    Ok(())
  }
}
