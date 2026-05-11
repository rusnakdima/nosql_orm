use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use std::time::Instant;

pub struct ReplicationConfig<S, T>
where
    S: DatabaseProvider,
    T: DatabaseProvider,
{
    pub source_provider: S,
    pub target_provider: T,
    pub collections: Vec<String>,
    pub mode: ReplicationMode,
    pub conflict_resolution: ConflictResolution,
}

pub enum ReplicationMode {
    FullSync,
    Incremental,
    Cdc,
}

pub enum ConflictResolution {
    SourceWins,
    TargetWins,
    LatestWins,
    Manual,
}

pub struct Replication<S, T>
where
    S: DatabaseProvider,
    T: DatabaseProvider,
{
    config: ReplicationConfig<S, T>,
}

impl<S, T> Replication<S, T>
where
    S: DatabaseProvider,
    T: DatabaseProvider,
{
    pub fn new(config: ReplicationConfig<S, T>) -> Self {
        Self { config }
    }

    pub async fn sync(&self) -> OrmResult<ReplicationResult> {
        match self.config.mode {
            ReplicationMode::FullSync => self.full_sync().await,
            ReplicationMode::Incremental => self.incremental_sync().await,
            ReplicationMode::Cdc => self.cdc_sync().await,
        }
    }

    async fn full_sync(&self) -> OrmResult<ReplicationResult> {
        let start = Instant::now();
        let mut synced_collections = 0;
        let mut synced_documents = 0;
        let mut conflicts = 0;

        for collection in &self.config.collections {
            let docs = self.config.source_provider.find_all(collection).await?;
            let target_docs = self.config.target_provider.find_all(collection).await?;

            let target_ids: std::collections::HashSet<String> = target_docs
                .iter()
                .filter_map(|d| d.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect();

            for doc in docs {
                let id = doc.get("id").and_then(|v| v.as_str());
                if let Some(id) = id {
                    let doc_clone = doc.clone();
                    if target_ids.contains(id) {
                        conflicts += 1;
                        match self.config.conflict_resolution {
                            ConflictResolution::SourceWins => {
                                self.config.target_provider.update(collection, id, doc_clone).await?;
                            }
                            ConflictResolution::TargetWins => {}
                            ConflictResolution::LatestWins => {
                                self.config.target_provider.update(collection, id, doc_clone).await?;
                            }
                            ConflictResolution::Manual => {}
                        }
                    } else {
                        self.config.target_provider.insert(collection, doc_clone).await?;
                    }
                    synced_documents += 1;
                }
            }
            synced_collections += 1;
        }

        Ok(ReplicationResult {
            synced_collections,
            synced_documents,
            conflicts,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn incremental_sync(&self) -> OrmResult<ReplicationResult> {
        let start = Instant::now();
        let mut synced_collections = 0;
        let mut synced_documents = 0;
        let mut conflicts = 0;

        for collection in &self.config.collections {
            let source_docs = self.config.source_provider.find_all(collection).await?;
            let target_docs = self.config.target_provider.find_all(collection).await?;

            let target_ids: std::collections::HashMap<String, serde_json::Value> = target_docs
                .into_iter()
                .filter_map(|d| {
                    let id = d.get("id")?.as_str()?.to_string();
                    Some((id, d))
                })
                .collect();

            for doc in source_docs {
                let id = doc.get("id").and_then(|v| v.as_str());
                if let Some(id) = id {
                    let doc_clone = doc.clone();
                    if target_ids.get(id).is_some() {
                        conflicts += 1;
                        match self.config.conflict_resolution {
                            ConflictResolution::SourceWins => {
                                self.config.target_provider.update(collection, id, doc_clone).await?;
                            }
                            ConflictResolution::TargetWins => {}
                            ConflictResolution::LatestWins => {
                                self.config.target_provider.update(collection, id, doc_clone).await?;
                            }
                            ConflictResolution::Manual => {}
                        }
                    } else {
                        self.config.target_provider.insert(collection, doc_clone).await?;
                    }
                    synced_documents += 1;
                }
            }
            synced_collections += 1;
        }

        Ok(ReplicationResult {
            synced_collections,
            synced_documents,
            conflicts,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn cdc_sync(&self) -> OrmResult<ReplicationResult> {
        let start = Instant::now();
        let mut synced_collections = 0;
        let mut synced_documents = 0;
        let mut conflicts = 0;

        for collection in &self.config.collections {
            let source_docs = self.config.source_provider.find_all(collection).await?;

            for doc in source_docs {
                let id = doc.get("id").and_then(|v| v.as_str());
                if let Some(id) = id {
                    let doc_clone = doc.clone();
                    let exists = self.config.target_provider.exists(collection, id).await?;

                    if exists {
                        conflicts += 1;
                        match self.config.conflict_resolution {
                            ConflictResolution::SourceWins => {
                                self.config.target_provider.update(collection, id, doc_clone).await?;
                            }
                            ConflictResolution::TargetWins => {}
                            ConflictResolution::LatestWins => {
                                self.config.target_provider.update(collection, id, doc_clone).await?;
                            }
                            ConflictResolution::Manual => {}
                        }
                    } else {
                        self.config.target_provider.insert(collection, doc_clone).await?;
                    }
                    synced_documents += 1;
                }
            }
            synced_collections += 1;
        }

        Ok(ReplicationResult {
            synced_collections,
            synced_documents,
            conflicts,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[derive(Debug)]
pub struct ReplicationResult {
    pub synced_collections: usize,
    pub synced_documents: usize,
    pub conflicts: usize,
    pub duration_ms: u64,
}

impl Default for ReplicationResult {
    fn default() -> Self {
        Self {
            synced_collections: 0,
            synced_documents: 0,
            conflicts: 0,
            duration_ms: 0,
        }
    }
}