use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::Filter;
use serde_json::Value;

pub struct ResolverContext {
  pub request_id: String,
  pub variables: Value,
}

#[async_trait::async_trait]
pub trait GraphQLResolver<E = Value>: Send + Sync {
  async fn resolve_query(&self, ctx: &ResolverContext, info: &QueryInfo) -> OrmResult<Value>;
  async fn resolve_mutation(&self, ctx: &ResolverContext, info: &MutationInfo) -> OrmResult<Value>;
}

#[derive(Debug, Clone)]
pub struct QueryInfo {
  pub field_name: String,
  pub args: Value,
  pub selection_set: Vec<String>,
  pub collection: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MutationInfo {
  pub field_name: String,
  pub args: Value,
  pub input: Value,
}

pub struct SimpleResolver;

#[async_trait::async_trait]
impl GraphQLResolver for SimpleResolver {
  async fn resolve_query(&self, _ctx: &ResolverContext, info: &QueryInfo) -> OrmResult<Value> {
    Ok(serde_json::json!({
        "data": {
            info.field_name.clone(): null
        }
    }))
  }

  async fn resolve_mutation(
    &self,
    _ctx: &ResolverContext,
    info: &MutationInfo,
  ) -> OrmResult<Value> {
    Ok(serde_json::json!({
        "data": {
            info.field_name.clone(): null
        }
    }))
  }
}

pub struct ProviderResolver<P: DatabaseProvider> {
  provider: P,
}

impl<P: DatabaseProvider> ProviderResolver<P> {
  pub fn new(provider: P) -> Self {
    Self { provider }
  }

  pub async fn resolve_query(&self, info: &QueryInfo) -> OrmResult<Vec<Value>> {
    let collection = info
      .collection
      .clone()
      .unwrap_or_else(|| info.field_name.clone());

    let filter = if let Some(where_arg) = info.args.get("where") {
      Some(Filter::from_json(where_arg)?)
    } else {
      None
    };

    let limit = info.args.get("limit").and_then(|v| v.as_u64());
    let skip = info.args.get("skip").and_then(|v| v.as_u64());

    let results = self
      .provider
      .find_many(&collection, filter.as_ref(), skip, limit, None, true)
      .await?;

    Ok(results)
  }

  pub async fn resolve_create(&self, collection: &str, data: Value) -> OrmResult<Value> {
    self.provider.insert(collection, data).await
  }

  pub async fn resolve_update(&self, collection: &str, id: &str, data: Value) -> OrmResult<Value> {
    self.provider.update(collection, id, data).await
  }

  pub async fn resolve_delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self.provider.delete(collection, id).await
  }
}
