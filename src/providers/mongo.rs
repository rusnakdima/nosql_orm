use async_trait::async_trait;
use mongodb::{
  bson::{doc, from_bson, to_bson, Bson, Document},
  options::{ClientOptions, FindOptions},
  Client, Database,
};
use serde_json::Value;

use crate::error::{OrmError, OrmResult};
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

  // ── Conversions ────────────────────────────────────────────────────────

  fn json_to_bson(value: Value) -> OrmResult<Document> {
    let bson = to_bson(&value)
      .map_err(|e| OrmError::Serialization(serde_json::Error::custom(e.to_string())))?;
    bson
      .as_document()
      .cloned()
      .ok_or_else(|| OrmError::Provider("Expected BSON document".to_string()))
  }

  fn bson_to_json(doc: Document) -> OrmResult<Value> {
    let bson = Bson::Document(doc);
    let json: Value = from_bson(bson)
      .map_err(|e| OrmError::Serialization(serde_json::Error::custom(e.to_string())))?;
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

    let query = filter.map(Self::filter_to_doc).unwrap_or_default();
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
    let query = filter.map(Self::filter_to_doc).unwrap_or_default();
    let coll = self.db.collection::<Document>(collection);
    coll.count_documents(query, None).await.map_err(Into::into)
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
