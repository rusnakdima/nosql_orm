use crate::error::OrmResult;
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
