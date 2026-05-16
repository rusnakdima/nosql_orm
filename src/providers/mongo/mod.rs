use crate::error::{map_err_connection, OrmError, OrmResult};
use crate::nosql_index::{NosqlIndex, NosqlIndexInfo};
use crate::provider::{
  AdminCommands, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  DatabaseProvider, FieldInfo, IndexInfo, ProviderConfig, RawResult, SchemaIntrospection,
  TransactionControl, TransactionId,
};
use crate::query::Filter;
use crate::utils::generate_id;
use async_trait::async_trait;
use mongodb::{
  bson::{doc, Bson, Document},
  options::FindOptions,
  Database,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod convert;
mod filter;
mod helpers;
mod indexes;

use crate::providers::mongo::convert::{bson_to_json, json_to_bson};
use crate::providers::mongo::filter::filter_to_doc;
use crate::providers::mongo::indexes::{build_index_keys, build_index_options};

#[derive(Clone)]
pub struct MongoProvider {
  db: Database,
  transaction_manager: Arc<Mutex<Option<TransactionId>>>,
}

impl MongoProvider {
  pub async fn connect(uri: impl AsRef<str>, db_name: impl AsRef<str>) -> OrmResult<Self> {
    let options = map_err_connection(mongodb::options::ClientOptions::parse(uri.as_ref()).await)?;
    let client = map_err_connection(mongodb::Client::with_options(options))?;

    Ok(Self {
      db: client.database(db_name.as_ref()),
      transaction_manager: Arc::new(Mutex::new(None)),
    })
  }

  pub async fn from_config(config: &ProviderConfig) -> OrmResult<Self> {
    let db_name = config.database.as_deref().unwrap_or("nosql_orm");
    Self::connect(&config.connection, db_name).await
  }

  pub async fn create_mongo_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    let keys = build_index_keys(index);
    let opts = build_index_options(index);
    let model = mongodb::IndexModel::builder()
      .keys(keys)
      .options(opts)
      .build();
    let coll = self.db.collection::<Document>(collection);
    coll.create_index(model, None).await?;
    Ok(())
  }

  pub async fn drop_mongo_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    let coll = self.db.collection::<Document>(collection);
    coll.drop_index(index_name, None).await?;
    Ok(())
  }

  async fn list_mongo_indexes(&self, collection: &str) -> OrmResult<Vec<NosqlIndexInfo>> {
    use futures_util::TryStreamExt;
    let coll = self.db.collection::<Document>(collection);
    let mut cursor = coll.list_indexes(None).await?;
    let mut indexes = Vec::new();
    while let Some(idx) = cursor.try_next().await? {
      let name = idx
        .options
        .as_ref()
        .and_then(|o| o.name.clone())
        .unwrap_or_default();
      let namespace = format!("{}.{}", self.db.name(), collection);
      let mut fields = Vec::new();
      for (k, v) in &idx.keys {
        let order = match v {
          Bson::Int32(i) => *i,
          Bson::Int64(i) => *i as i32,
          Bson::String(s) if s == "text" => 1i32,
          _ => 1i32,
        };
        fields.push((k.to_string(), order));
      }
      let opts = idx.options.as_ref();
      let unique = opts.and_then(|o| o.unique).unwrap_or(false);
      let sparse = opts.and_then(|o| o.sparse).unwrap_or(false);
      let version = opts
        .and_then(|o| o.version.clone())
        .map(|v| format!("{:?}", v));
      let expire_secs = opts
        .and_then(|o| o.expire_after.as_ref())
        .map(|d| d.as_secs() as u32);
      let index_type = if opts.and_then(|o| o.text_index_version.clone()).is_some() {
        "text"
      } else if fields.len() > 1 {
        "compound"
      } else {
        "single"
      };
      indexes.push(NosqlIndexInfo {
        name,
        namespace,
        unique,
        sparse,
        ttl_seconds: expire_secs,
        version,
        index_type: index_type.to_string(),
        fields,
      });
    }
    Ok(indexes)
  }

  fn json_to_bson(value: Value) -> OrmResult<Document> {
    json_to_bson(value)
  }

  fn bson_to_json(doc: Document) -> OrmResult<Value> {
    bson_to_json(doc)
  }

  fn filter_to_doc(filter: &Filter) -> Document {
    filter_to_doc(filter)
  }
}

#[async_trait]
impl DatabaseProvider for MongoProvider {
  async fn insert(&self, collection: &str, mut doc: Value) -> OrmResult<Value> {
    if doc
      .get("id")
      .and_then(|v| v.as_str())
      .is_none_or(|s| s.is_empty())
    {
      doc["id"] = serde_json::json!(generate_id());
    }
    let coll = self.db.collection::<Document>(collection);
    let mut bson_doc = Self::json_to_bson(doc.clone())?;
    if let Some(id) = bson_doc.remove("id") {
      bson_doc.insert("_id", id);
    }
    coll.insert_one(bson_doc, None).await?;
    Ok(doc)
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let coll = self.db.collection::<Document>(collection);
    let found = coll.find_one(doc! { "_id": id }, None).await?;
    found.map(Self::bson_to_json).transpose()
  }

  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<Value>> {
    use futures_util::TryStreamExt;
    let query = filter
      .as_ref()
      .map(|f| Self::filter_to_doc(f))
      .unwrap_or_default();
    let mut opts = FindOptions::default();
    opts.skip = skip;
    opts.limit = limit.map(|n| n as i64);
    if let Some(field) = sort_by {
      opts.sort = Some(doc! { field: if sort_asc { 1i32 } else { -1i32 } });
    }
    let coll = self.db.collection::<Document>(collection);
    let mut cursor = coll.find(query, opts).await?;
    let mut results = vec![];
    while let Some(doc) = cursor.try_next().await? {
      results.push(Self::bson_to_json(doc)?);
    }
    Ok(results)
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let coll = self.db.collection::<Document>(collection);
    let mut bson_doc = Self::json_to_bson(doc.clone())?;
    bson_doc.remove("id");
    bson_doc.remove("_id");
    coll.replace_one(doc! { "_id": id }, bson_doc, None).await?;
    Ok(doc)
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let coll = self.db.collection::<Document>(collection);
    let patch_doc = Self::json_to_bson(patch)?;
    coll
      .update_one(doc! { "_id": id }, doc! { "$set": patch_doc }, None)
      .await?;
    self
      .find_by_id(collection, id)
      .await?
      .ok_or_else(|| OrmError::NotFound(format!("{}/{}", collection, id)))
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    let coll = self.db.collection::<Document>(collection);
    let res = coll.delete_one(doc! { "_id": id }, None).await?;
    Ok(res.deleted_count > 0)
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let query = filter
      .as_ref()
      .map(|f| Self::filter_to_doc(f))
      .unwrap_or_default();
    let coll = self.db.collection::<Document>(collection);
    coll.count_documents(query, None).await.map_err(Into::into)
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    let coll = self.db.collection::<Document>(collection);
    let query = filter
      .as_ref()
      .map(|f| Self::filter_to_doc(f))
      .unwrap_or_default();
    let update_doc = Self::json_to_bson(updates)?;
    let result = coll
      .update_many(query, doc! { "$set": update_doc }, None)
      .await?;
    Ok(result.modified_count as usize)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let coll = self.db.collection::<Document>(collection);
    let query = filter
      .as_ref()
      .map(|f| Self::filter_to_doc(f))
      .unwrap_or_default();
    let result = coll.delete_many(query, None).await?;
    Ok(result.deleted_count as usize)
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    self.create_mongo_index(collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.drop_mongo_index(collection, index_name).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    let indexes = self.list_mongo_indexes(collection).await?;
    Ok(
      indexes
        .into_iter()
        .map(|idx| IndexInfo {
          name: idx.name,
          collection: idx.namespace,
          fields: idx.fields.into_iter().map(|(f, _)| f).collect(),
          index_type: idx.index_type,
          unique: idx.unique,
          sparse: idx.sparse,
        })
        .collect(),
    )
  }

  async fn health_check(&self) -> OrmResult<bool> {
    self
      .db
      .run_command(doc! { "ping": 1 }, None)
      .await
      .map(|_| true)
      .map_err(Into::into)
  }

  async fn insert_many(&self, collection: &str, docs: Vec<Value>) -> OrmResult<usize> {
    let coll = self.db.collection::<Document>(collection);
    let mut bson_docs = Vec::new();
    for mut doc in docs {
      if doc
        .get("id")
        .and_then(|v| v.as_str())
        .is_none_or(|s| s.is_empty())
      {
        doc["id"] = serde_json::json!(generate_id());
      }
      let mut bson_doc = Self::json_to_bson(doc)?;
      if let Some(id) = bson_doc.remove("id") {
        bson_doc.insert("_id", id);
      }
      bson_docs.push(bson_doc);
    }
    let count = bson_docs.len();
    if !bson_docs.is_empty() {
      coll.insert_many(bson_docs, None).await?;
    }
    Ok(count)
  }
}

#[async_trait]
impl SchemaIntrospection for MongoProvider {
  async fn list_collections(&self) -> OrmResult<Vec<CollectionMeta>> {
    let names = self.db.list_collection_names(None).await?;
    let mut collections = Vec::new();
    for name in names {
      let coll = self.db.collection::<Document>(&name);
      let count = coll.count_documents(doc! {}, None).await.unwrap_or(0);
      collections.push(CollectionMeta {
        name,
        document_count: count,
        size_bytes: 0,
        created_at: None,
        updated_at: None,
      });
    }
    Ok(collections)
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<CollectionSchema> {
    let coll = self.db.collection::<Document>(collection);
    let sample = coll.find_one(doc! {}, None).await?;
    let mut fields = HashMap::new();
    if let Some(doc) = sample {
      for (k, v) in doc {
        let field_type = match v {
          Bson::Null => "null".to_string(),
          Bson::Boolean(_) => "boolean".to_string(),
          Bson::Int32(_) | Bson::Int64(_) | Bson::Double(_) => "number".to_string(),
          Bson::String(_) => "string".to_string(),
          Bson::Array(_) => "array".to_string(),
          Bson::Document(_) => "object".to_string(),
          Bson::Binary(_) => "binary".to_string(),
          Bson::ObjectId(_) => "objectid".to_string(),
          Bson::DateTime(_) => "datetime".to_string(),
          Bson::Timestamp(_) => "timestamp".to_string(),
          Bson::Decimal128(_) => "decimal128".to_string(),
          Bson::JavaScriptCode(_) | Bson::JavaScriptCodeWithScope(_) => "javascript".to_string(),
          _ => "unknown".to_string(),
        };
        fields.insert(
          k.clone(),
          FieldInfo {
            name: k,
            field_type,
            nullable: true,
            default_value: None,
          },
        );
      }
    }
    Ok(CollectionSchema {
      name: collection.to_string(),
      fields,
      indexes: vec![],
      options: Default::default(),
    })
  }

  async fn get_collection_stats(&self, collection: &str) -> OrmResult<CollectionStats> {
    let coll = self.db.collection::<Document>(collection);
    let count = coll.count_documents(doc! {}, None).await.unwrap_or(0);
    Ok(CollectionStats {
      name: collection.to_string(),
      document_count: count,
      size_bytes: 0,
      storage_size_bytes: 0,
      index_count: 0,
      index_size_bytes: 0,
      average_document_size: 0,
    })
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<IndexInfo>> {
    let indexes = self.list_mongo_indexes(collection).await?;
    Ok(
      indexes
        .into_iter()
        .map(|idx| IndexInfo {
          name: idx.name,
          collection: idx.namespace,
          fields: idx.fields.into_iter().map(|(f, _)| f).collect(),
          index_type: idx.index_type,
          unique: idx.unique,
          sparse: idx.sparse,
        })
        .collect(),
    )
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    Ok(self.db.name().to_string())
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    let result = self
      .db
      .run_command(doc! { "listDatabases": 1 }, None)
      .await?;
    let mut names = Vec::new();
    if let Some(databases) = result.get("databases").and_then(|v: &Bson| v.as_array()) {
      for db_info in databases {
        if let Bson::Document(doc) = db_info {
          if let Some(name) = doc.get("name").and_then(|v| v.as_str()) {
            names.push(name.to_string());
          }
        }
      }
    }
    Ok(names)
  }
}

#[async_trait]
impl AdminCommands for MongoProvider {
  async fn execute_raw(&self, _query: &str, _params: Vec<Value>) -> OrmResult<RawResult> {
    Err(OrmError::NotSupported(
      "Raw execution not supported for MongoDB provider".to_string(),
    ))
  }

  async fn create_collection(
    &self,
    collection: &str,
    _schema: Option<CollectionSchema>,
  ) -> OrmResult<()> {
    self.db.create_collection(collection, None).await?;
    Ok(())
  }

  async fn drop_collection(&self, collection: &str) -> OrmResult<()> {
    self
      .db
      .collection::<Document>(collection)
      .drop(None)
      .await?;
    Ok(())
  }

  async fn rename_collection(&self, from: &str, to: &str) -> OrmResult<()> {
    self
      .db
      .run_command(doc! { "renameCollection": from, "to": to }, None)
      .await?;
    Ok(())
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    let build_info = self.db.run_command(doc! { "buildInfo": 1 }, None).await?;
    Ok(
      build_info
        .get_str("version")
        .unwrap_or("unknown")
        .to_string(),
    )
  }

  async fn health_check_detailed(&self) -> OrmResult<ConnectionHealth> {
    let healthy = self.health_check().await.unwrap_or(false);
    let server_version = self
      .get_server_version()
      .await
      .unwrap_or_else(|_| "unknown".to_string());
    Ok(ConnectionHealth {
      healthy,
      latency_ms: None,
      server_version: Some(server_version),
      connected_at: None,
      pool_stats: None,
    })
  }
}

#[async_trait]
impl TransactionControl for MongoProvider {
  async fn begin_transaction(&self) -> OrmResult<TransactionId> {
    let mut guard = self.transaction_manager.lock().unwrap();
    if guard.is_some() {
      return Err(OrmError::Transaction(
        "Transaction already active".to_string(),
      ));
    }
    let id = TransactionId::new(uuid::Uuid::new_v4().to_string());
    *guard = Some(id.clone());
    Ok(id)
  }

  async fn commit_transaction(&self, id: TransactionId) -> OrmResult<()> {
    let mut guard = self.transaction_manager.lock().unwrap();
    match guard.as_ref() {
      Some(active_id) if active_id == &id => {
        *guard = None;
        Ok(())
      }
      Some(_) => Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
      None => Err(OrmError::Transaction("No active transaction".to_string())),
    }
  }

  async fn rollback_transaction(&self, id: TransactionId) -> OrmResult<()> {
    let mut guard = self.transaction_manager.lock().unwrap();
    match guard.as_ref() {
      Some(active_id) if active_id == &id => {
        *guard = None;
        Ok(())
      }
      Some(_) => Err(OrmError::Transaction("Transaction ID mismatch".to_string())),
      None => Err(OrmError::Transaction("No active transaction".to_string())),
    }
  }

  async fn is_transaction_active(&self) -> OrmResult<bool> {
    Ok(self.transaction_manager.lock().unwrap().is_some())
  }
}
