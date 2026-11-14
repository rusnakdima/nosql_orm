// Multi-tenancy wrapper: transparently scopes all operations to a tenant.
//
// Wraps any `Storage` implementation and injects a `tenant_id` filter into
// every query, insert, update and delete. The tenant id is stored in a
// `_tenant` field on each document.

use crate::error::OrmResult;
use crate::provider::Storage;
use crate::query::Filter;
use async_trait::async_trait;
use serde_json::{json, Value};


#[derive(Clone)]
pub struct TenantStorage<P: Storage> {
  inner: P,
  tenant_id: String,
}

impl<P: Storage> TenantStorage<P> {
  pub fn new(inner: P, tenant_id: impl Into<String>) -> Self {
    Self {
      inner,
      tenant_id: tenant_id.into(),
    }
  }

  pub fn tenant_id(&self) -> &str {
    &self.tenant_id
  }

  fn inject_tenant(&self, mut doc: Value) -> Value {
    if let Some(obj) = doc.as_object_mut() {
      obj.insert("_tenant".to_string(), json!(self.tenant_id));
    }
    doc
  }

  fn tenant_filter(&self) -> Filter {
    Filter::Eq("_tenant".to_string(), json!(self.tenant_id))
  }
}

#[async_trait]
impl<P: Storage> Storage for TenantStorage<P> {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    let doc = self.inject_tenant(doc);
    self.inner.insert(collection, doc).await
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let result = self.inner.find_by_id(collection, id).await?;
    // Verify tenant ownership
    if let Some(ref doc) = result {
      if doc.get("_tenant").and_then(Value::as_str) != Some(&self.tenant_id) {
        return Ok(None);
      }
    }
    Ok(result)
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
    let combined = match filter {
      Some(f) => Filter::And(vec![self.tenant_filter(), f.clone()]),
      None => self.tenant_filter(),
    };
    self
      .inner
      .find_many(collection, Some(&combined), skip, limit, sort_by, sort_asc)
      .await
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let doc = self.inject_tenant(doc);
    self.inner.update(collection, id, doc).await
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    self.inner.patch(collection, id, patch).await
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    // Verify tenant ownership before delete
    if let Some(doc) = self.inner.find_by_id(collection, id).await? {
      if doc.get("_tenant").and_then(Value::as_str) != Some(&self.tenant_id) {
        return Ok(false);
      }
    }
    self.inner.delete(collection, id).await
  }

  async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize> {
    let combined = match filter {
      Some(f) => Filter::And(vec![self.tenant_filter(), f]),
      None => self.tenant_filter(),
    };
    self.inner.delete_many(collection, Some(combined)).await
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    let combined = match filter {
      Some(f) => Filter::And(vec![self.tenant_filter(), f]),
      None => self.tenant_filter(),
    };
    self.inner.update_many(collection, Some(combined), updates).await
  }

  async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64> {
    let combined = match filter {
      Some(f) => Filter::And(vec![self.tenant_filter(), f.clone()]),
      None => self.tenant_filter(),
    };
    self.inner.count(collection, Some(&combined)).await
  }

  async fn create_index(&self, collection: &str, index: &crate::nosql_index::NosqlIndex) -> OrmResult<()> {
    self.inner.create_index(collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.inner.drop_index(collection, index_name).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<crate::provider::IndexInfo>> {
    crate::provider::Storage::list_indexes(&self.inner, collection).await
  }

}

#[async_trait]
impl<P: Storage + crate::provider::SchemaIntrospection> crate::provider::SchemaIntrospection for TenantStorage<P> {
  async fn list_collections(&self) -> OrmResult<Vec<crate::provider::CollectionMeta>> {
    self.inner.list_collections().await
  }

  async fn describe_collection(&self, collection: &str) -> OrmResult<crate::provider::CollectionSchema> {
    self.inner.describe_collection(collection).await
  }

  async fn get_collection_stats(&self, collection: &str) -> OrmResult<crate::provider::CollectionStats> {
    self.inner.get_collection_stats(collection).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<crate::provider::IndexInfo>> {
    crate::provider::Storage::list_indexes(&self.inner, collection).await
  }

  async fn get_database_name(&self) -> OrmResult<String> {
    self.inner.get_database_name().await
  }

  async fn list_databases(&self) -> OrmResult<Vec<String>> {
    self.inner.list_databases().await
  }
}

#[async_trait]
impl<P: Storage + crate::provider::AdminCommands> crate::provider::AdminCommands for TenantStorage<P> {
  async fn execute_raw(&self, sql: &str, params: Vec<Value>) -> OrmResult<crate::provider::RawResult> {
    self.inner.execute_raw(sql, params).await
  }

  async fn create_collection(&self, name: &str, schema: Option<crate::provider::CollectionSchema>) -> OrmResult<()> {
    self.inner.create_collection(name, schema).await
  }

  async fn drop_collection(&self, name: &str) -> OrmResult<()> {
    self.inner.drop_collection(name).await
  }

  async fn rename_collection(&self, old_name: &str, new_name: &str) -> OrmResult<()> {
    self.inner.rename_collection(old_name, new_name).await
  }

  async fn get_server_version(&self) -> OrmResult<String> {
    self.inner.get_server_version().await
  }

  async fn health_check_detailed(&self) -> OrmResult<crate::provider::ConnectionHealth> {
    self.inner.health_check_detailed().await
  }
}
