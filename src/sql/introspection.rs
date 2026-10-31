//! Table introspection for the SQL providers.
//!
//! A projection that *excludes* fields cannot be rendered from the
//! projection alone: the remaining columns are the table's columns minus the
//! excluded ones, and only the database knows the table's columns. Providers
//! implement [`TableIntrospector`] to report them, and the query path asks
//! the database first and hands the real column list to the builder.

use crate::error::OrmResult;
use crate::query::{Filter, OrderBy, Projection};
use crate::sql::query::SqlQueryBuilder;
use async_trait::async_trait;

/// Reports the real columns of a table.
#[async_trait]
pub trait TableIntrospector: Send + Sync {
  /// The columns of `table`, in the order the database reports them.
  async fn table_columns(&self, table: &str) -> OrmResult<Vec<String>>;
}

/// The table's columns, but only when the projection needs them.
///
/// A `select` projection names its columns, so nothing has to be asked of the
/// database. An `exclude` projection names the columns to drop, so the rest
/// are only known once the table's own columns are.
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

/// Renders a `SELECT` statement for `table`.
///
/// The columns an `exclude` projection leaves behind are the table's real
/// columns minus the excluded ones, so the database is asked for them before
/// the statement is built.
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
