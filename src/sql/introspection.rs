use crate::error::OrmResult;
use crate::query::{Filter, OrderBy, Projection};
use crate::sql::query::SqlQueryBuilder;
use async_trait::async_trait;

#[async_trait]
pub trait TableIntrospector: Send + Sync {
  async fn table_columns(&self, table: &str) -> OrmResult<Vec<String>>;
}

pub async fn projection_columns(
  introspector: &dyn TableIntrospector,
  table: &str,
  projection: Option<&Projection>,
) -> OrmResult<Option<Vec<String>>> {
  let excludes = match projection {
    Some(p) => p.select.is_none() && p.exclude.is_some(),
    None => false,
  };
  if !excludes {
    return Ok(None);
  }
  Ok(Some(introspector.table_columns(table).await?))
}

pub async fn build_select(
  introspector: &dyn TableIntrospector,
  builder: &SqlQueryBuilder,
  table: &str,
  filter: Option<&Filter>,
  projection: Option<&Projection>,
  order_by: Option<&[OrderBy]>,
  limit: Option<u32>,
  offset: Option<u64>,
) -> OrmResult<String> {
  let all_columns = projection_columns(introspector, table, projection).await?;
  builder.build_select(
    table,
    filter,
    projection,
    order_by,
    limit,
    offset,
    all_columns.as_deref(),
  )
}
