use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionMeta {
  pub name: String,
  pub document_count: u64,
  pub size_bytes: u64,
  pub created_at: Option<String>,
  pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSchema {
  pub name: String,
  pub fields: HashMap<String, FieldInfo>,
  pub indexes: Vec<IndexInfo>,
  pub options: CollectionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
  pub name: String,
  pub field_type: String,
  pub nullable: bool,
  pub default_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionOptions {
  pub validation_level: Option<String>,
  pub validation_action: Option<String>,
  pub expire_after_seconds: Option<u64>,
}

impl Default for CollectionOptions {
  fn default() -> Self {
    Self {
      validation_level: None,
      validation_action: None,
      expire_after_seconds: None,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionStats {
  pub name: String,
  pub document_count: u64,
  pub size_bytes: u64,
  pub storage_size_bytes: u64,
  pub index_count: u64,
  pub index_size_bytes: u64,
  pub average_document_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
  pub name: String,
  pub collection: String,
  pub fields: Vec<String>,
  pub index_type: String,
  pub unique: bool,
  pub sparse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawResult {
  pub columns: Vec<String>,
  pub rows: Vec<Vec<serde_json::Value>>,
  pub affected_rows: u64,
  pub last_insert_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionHealth {
  pub healthy: bool,
  pub latency_ms: Option<u64>,
  pub server_version: Option<String>,
  pub connected_at: Option<String>,
  pub pool_stats: Option<PoolStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
  pub total_connections: u32,
  pub idle_connections: u32,
  pub active_connections: u32,
  pub waiting_requests: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionId(pub String);

impl TransactionId {
  pub fn new(id: impl Into<String>) -> Self {
    Self(id.into())
  }
}

impl std::fmt::Display for TransactionId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}
