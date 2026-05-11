use crate::entity::Entity;
use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
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
        P: DatabaseProvider,
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
        P: DatabaseProvider,
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
        P: DatabaseProvider,
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
        P: DatabaseProvider,
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
        P: DatabaseProvider,
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
        P: DatabaseProvider,
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