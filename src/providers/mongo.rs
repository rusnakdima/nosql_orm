use async_trait::async_trait;
use mongodb::{
  bson::{doc, from_bson, to_bson, Bson, Document},
  options::{ClientOptions, DeleteOptions, FindOptions, IndexOptions, UpdateOptions},
  Client, Database, IndexModel,
};
use serde::{ser::Error, Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use crate::error::{OrmError, OrmResult};
use crate::nosql_index::{NosqlIndex, NosqlIndexInfo, NosqlIndexType};
use crate::provider::{DatabaseProvider, ProviderConfig};
use crate::query::Filter;
use crate::utils::generate_id;

/// MongoDB-backed provider.
///
/// Connect via `MongoProvider::connect("mongodb://localhost:27017", "mydb")`.
#[derive(Clone)]
pub struct MongoProvider {
  db: Database,
}

impl MongoProvider {
  /// Create a new provider connected to the given URI and database name.
  pub async fn connect(uri: impl AsRef<str>, db_name: impl AsRef<str>) -> OrmResult<Self> {
    let options = ClientOptions::parse(uri.as_ref())
      .await
      .map_err(|e| OrmError::Connection(e.to_string()))?;

    let client = Client::with_options(options).map_err(|e| OrmError::Connection(e.to_string()))?;

    Ok(Self {
      db: client.database(db_name.as_ref()),
    })
  }

  /// Create from a `ProviderConfig`.
  pub async fn from_config(config: &ProviderConfig) -> OrmResult<Self> {
    let db_name = config.database.as_deref().unwrap_or("nosql_orm");
    Self::connect(&config.connection, db_name).await
  }

  // ── Index Management ────────────────────────────────────────────────────

  /// Build MongoDB index keys document from a NosqlIndex.
  fn build_index_keys(index: &NosqlIndex) -> Document {
    let mut doc = Document::new();
    for (field, order) in index.get_fields() {
      let value: Bson = match index.get_index_type() {
        NosqlIndexType::Geospatial2dsphere => Bson::Int32(1), // MongoDB uses 1 for 2dsphere
        NosqlIndexType::Geospatial2d => Bson::Int32(1),
        NosqlIndexType::Text => Bson::String("text".to_string()), // Text index uses "text"
        NosqlIndexType::Hashed => Bson::String("hashed".to_string()),
        _ => Bson::Int32(*order),
      };
      doc.insert(field, value);
    }
    doc
  }

  /// Build MongoDB index options from a NosqlIndex.
  fn build_index_options(index: &NosqlIndex) -> IndexOptions {
    let mut opts = IndexOptions::default();

    if let Some(name) = index.get_name() {
      opts.name = Some(name.to_string());
    }

    if index.is_unique() {
      opts.unique = Some(true);
    }

    if index.is_sparse() {
      opts.sparse = Some(true);
    }

    if let Some(ttl) = index.get_ttl_seconds() {
      opts.expire_after = Some(Duration::from_secs(ttl as u64));
    }

    if let Some(ref partial_filter) = index.get_partial_filter() {
      opts.partial_filter_expression = Some(Self::filter_to_doc(partial_filter));
    }

    if let Some(ref weights) = index.get_weights() {
      let mut doc = Document::new();
      for (field, weight) in weights.iter() {
        doc.insert(field, *weight);
      }
      opts.weights = Some(doc);
    }

    if let Some(lang) = index.get_default_language() {
      opts.default_language = Some(lang.to_string());
    }

    opts
  }

  /// Create a MongoDB index.
  pub async fn create_mongo_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    let keys = Self::build_index_keys(index);
    let opts = Self::build_index_options(index);

    let model = IndexModel::builder().keys(keys).options(opts).build();

    let coll = self.db.collection::<Document>(collection);
    coll.create_index(model, None).await?;

    Ok(())
  }

  /// Drop a MongoDB index by name.
  pub async fn drop_mongo_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    let coll = self.db.collection::<Document>(collection);
    coll.drop_index(index_name, None).await?;
    Ok(())
  }

  /// List all MongoDB indexes on a collection.
  pub async fn list_mongo_indexes(&self, collection: &str) -> OrmResult<Vec<NosqlIndexInfo>> {
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
          Bson::Int32(i) => *i as i32,
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

  // ── Conversions ────────────────────────────────────────────────────────

  fn json_to_bson(value: Value) -> OrmResult<Document> {
    let bson = to_bson(&value)
      .map_err(|e| OrmError::Serialization(serde::ser::Error::custom(e.to_string())))?;
    bson
      .as_document()
      .cloned()
      .ok_or_else(|| OrmError::Provider("Expected BSON document".to_string()))
  }

  fn bson_to_json(doc: Document) -> OrmResult<Value> {
    let bson = Bson::Document(doc);
    let json: Value = from_bson(bson)
      .map_err(|e| OrmError::Serialization(serde::ser::Error::custom(e.to_string())))?;
    // Rename MongoDB's `_id` to `id` for API uniformity
    Ok(normalize_id(json))
  }

  fn filter_to_doc(filter: &Filter) -> Document {
    match filter {
      Filter::Eq(f, v) => doc! { f: to_bson(v).unwrap_or(Bson::Null) },
      Filter::Ne(f, v) => doc! { f: { "$ne": to_bson(v).unwrap_or(Bson::Null) } },
      Filter::Gt(f, v) => doc! { f: { "$gt": to_bson(v).unwrap_or(Bson::Null) } },
      Filter::Gte(f, v) => doc! { f: { "$gte": to_bson(v).unwrap_or(Bson::Null) } },
      Filter::Lt(f, v) => doc! { f: { "$lt": to_bson(v).unwrap_or(Bson::Null) } },
      Filter::Lte(f, v) => doc! { f: { "$lte": to_bson(v).unwrap_or(Bson::Null) } },
      Filter::In(f, vals) => {
        let bson_vals: Vec<Bson> = vals
          .iter()
          .map(|v| to_bson(v).unwrap_or(Bson::Null))
          .collect();
        doc! { f: { "$in": bson_vals } }
      }
      Filter::NotIn(f, vals) => {
        let bson_vals: Vec<Bson> = vals
          .iter()
          .map(|v| to_bson(v).unwrap_or(Bson::Null))
          .collect();
        doc! { f: { "$nin": bson_vals } }
      }
      Filter::Contains(f, sub) => doc! { f: { "$regex": sub, "$options": "i" } },
      Filter::StartsWith(f, prefix) => {
        doc! { f: { "$regex": format!("^{}", regex_escape(prefix)), "$options": "i" } }
      }
      Filter::And(filters) => {
        let docs: Vec<Bson> = filters
          .iter()
          .map(|f| Bson::Document(Self::filter_to_doc(f)))
          .collect();
        doc! { "$and": docs }
      }
      Filter::Or(filters) => {
        let docs: Vec<Bson> = filters
          .iter()
          .map(|f| Bson::Document(Self::filter_to_doc(f)))
          .collect();
        doc! { "$or": docs }
      }
      Filter::Not(inner) => doc! { "$nor": [Self::filter_to_doc(inner)] },
      Filter::IsNull(f) => doc! { f: { "$exists": false } },
      Filter::IsNotNull(f) => doc! { f: { "$exists": true, "$ne": Bson::Null } },
      Filter::Like(f, pattern) => doc! { f: { "$regex": pattern, "$options": "i" } },
      Filter::EndsWith(f, suffix) => {
        let escaped = regex_escape(suffix);
        doc! { f: { "$regex": format!(".*{}$", escaped), "$options": "i" } }
      }
      Filter::Between(f, min, max) => {
        doc! { f: { "$gte": to_bson(min).unwrap_or(Bson::Null), "$lte": to_bson(max).unwrap_or(Bson::Null) } }
      }
    }
  }
}

#[async_trait]
impl DatabaseProvider for MongoProvider {
  async fn insert(&self, collection: &str, mut doc: Value) -> OrmResult<Value> {
    if doc
      .get("id")
      .and_then(|v| v.as_str())
      .map_or(true, |s| s.is_empty())
    {
      doc["id"] = serde_json::json!(generate_id());
    }
    let coll = self.db.collection::<Document>(collection);
    let mut bson_doc = Self::json_to_bson(doc.clone())?;
    // Map `id` → `_id` for Mongo
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

    let query = filter.map(|f| Self::filter_to_doc(&f)).unwrap_or_default();
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
    let query = filter.map(|f| Self::filter_to_doc(&f)).unwrap_or_default();
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
    let query = filter.map(|f| Self::filter_to_doc(&f)).unwrap_or_default();
    let update_doc = Self::json_to_bson(updates)?;
    let result = coll
      .update_many(query, doc! { "$set": update_doc }, None)
      .await?;
    Ok(result.modified_count as usize)
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let coll = self.db.collection::<Document>(collection);
    let query = filter.map(|f| Self::filter_to_doc(&f)).unwrap_or_default();
    let result = coll.delete_many(query, None).await?;
    Ok(result.deleted_count as usize)
  }

  async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()> {
    self.create_mongo_index(collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.drop_mongo_index(collection, index_name).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<NosqlIndexInfo>> {
    self.list_mongo_indexes(collection).await
  }

  async fn aggregate(&self, collection: &str, pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
    use futures_util::TryStreamExt;

    let coll = self.db.collection::<Document>(collection);
    let pipeline_docs: Result<Vec<Document>, _> = pipeline
      .iter()
      .map(|v| {
        let doc = to_bson(v)
          .map_err(|e| OrmError::Serialization(serde::ser::Error::custom(e.to_string())))?;
        doc
          .as_document()
          .cloned()
          .ok_or_else(|| OrmError::Provider("Expected BSON document in pipeline".to_string()))
      })
      .collect();

    let mut cursor = coll.aggregate(pipeline_docs?, None).await?;
    let mut results = vec![];
    while let Some(doc) = cursor.try_next().await? {
      results.push(Self::bson_to_json(doc)?);
    }
    Ok(results)
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
        .map_or(true, |s| s.is_empty())
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

// ── Helpers ──────────────────────────────────────────────────────────────────

fn normalize_id(mut v: Value) -> Value {
  if let Some(obj) = v.as_object_mut() {
    if let Some(id) = obj.remove("_id") {
      obj.insert("id".to_string(), id);
    }
  }
  v
}

fn regex_escape(s: &str) -> String {
  s.chars()
    .flat_map(|c| {
      if "^$.*+?()[]{}|\\".contains(c) {
        vec!['\\', c]
      } else {
        vec![c]
      }
    })
    .collect()
}
