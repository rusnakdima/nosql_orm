use crate::entity::Entity;
use crate::error::OrmResult;
use crate::provider::DatabaseProvider;
use crate::query::{Cursor, Filter, OrderBy, PaginatedResult, QueryBuilder, SortDirection};
use serde_json::Value;

use super::Repository;

pub struct RepositoryQuery<'r, E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  repo: &'r Repository<E, P>,
  builder: QueryBuilder,
}

impl<'r, E, P> RepositoryQuery<'r, E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub fn where_eq(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.builder = self.builder.where_eq(field, value);
    self
  }
  pub fn where_ne(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.builder = self.builder.where_ne(field, value);
    self
  }
  pub fn where_gt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.builder = self.builder.where_gt(field, value);
    self
  }
  pub fn where_lt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.builder = self.builder.where_lt(field, value);
    self
  }
  pub fn where_contains(mut self, field: impl Into<String>, sub: impl Into<String>) -> Self {
    self.builder = self.builder.where_contains(field, sub);
    self
  }
  pub fn where_in(mut self, field: impl Into<String>, values: Vec<Value>) -> Self {
    self.builder = self.builder.where_in(field, values);
    self
  }
  pub fn order_by(mut self, order: OrderBy) -> Self {
    self.builder = self.builder.order_by(order);
    self
  }
  pub fn skip(mut self, n: u64) -> Self {
    self.builder = self.builder.skip(n);
    self
  }
  pub fn limit(mut self, n: u64) -> Self {
    self.builder = self.builder.limit(n);
    self
  }
  pub fn with_relation(mut self, name: impl Into<String>) -> Self {
    self.builder = self.builder.with_relation(name);
    self
  }
  pub fn filter(mut self, f: Filter) -> Self {
    self.builder = self.builder.filter(f);
    self
  }
  pub fn select(mut self, fields: &[&str]) -> Self {
    self.builder = self.builder.select(fields);
    self
  }
  pub fn exclude(mut self, fields: &[&str]) -> Self {
    self.builder = self.builder.exclude(fields);
    self
  }

  pub async fn find(self) -> OrmResult<Vec<E>> {
    let filter = self.builder.build_filter();
    let (sort_field, sort_asc) = match &self.builder.order {
      Some(o) => (Some(o.field.as_str()), o.direction == SortDirection::Asc),
      None => (None, true),
    };
    let docs = self
      .repo
      .provider
      .find_many(
        &E::table_name(),
        filter.as_ref(),
        self.builder.skip,
        self.builder.limit,
        sort_field,
        sort_asc,
      )
      .await?;

    docs.into_iter().map(E::from_value).collect()
  }

  pub async fn find_with_cursor(self, cursor: Option<Cursor>) -> OrmResult<PaginatedResult<E>> {
    let mut builder = self.builder;

    if let Some(c) = cursor {
      let cursor_filter = c.as_filter();
      builder = builder.filter(cursor_filter);
    }

    let filter = builder.build_filter();
    let (sort_field, sort_asc) = match &builder.order {
      Some(o) => (Some(o.field.as_str()), o.direction == SortDirection::Asc),
      None => (Some("id"), true),
    };

    let docs = self
      .repo
      .provider
      .find_many(
        &E::table_name(),
        filter.as_ref(),
        None,
        builder.limit,
        sort_field,
        sort_asc,
      )
      .await?;

    let has_more = docs.len() as u64 >= builder.limit.unwrap_or(0);
    let next_cursor = docs.last().and_then(|doc| {
      doc.get("id").and_then(|v| v.as_str()).map(|id| Cursor {
        last_id: id.to_string(),
        sort_field: sort_field.unwrap_or("id").to_string(),
        sort_asc,
      })
    });

    let entities: Vec<E> = docs
      .into_iter()
      .map(E::from_value)
      .collect::<Result<Vec<_>, _>>()?;

    Ok(PaginatedResult {
      data: entities,
      next_cursor,
      has_more,
    })
  }

  pub async fn find_one(self) -> OrmResult<Option<E>> {
    Ok(self.limit(1).find().await?.into_iter().next())
  }

  pub async fn find_raw(self) -> OrmResult<Vec<Value>> {
    let filter = self.builder.build_filter();
    let (sort_field, sort_asc) = match &self.builder.order {
      Some(o) => (Some(o.field.as_str()), o.direction == SortDirection::Asc),
      None => (None, true),
    };
    self
      .repo
      .provider
      .find_many(
        &E::table_name(),
        filter.as_ref(),
        self.builder.skip,
        self.builder.limit,
        sort_field,
        sort_asc,
      )
      .await
  }

  pub async fn count(self) -> OrmResult<u64> {
    let filter = self.builder.build_filter();
    self
      .repo
      .provider
      .count(&E::table_name(), filter.as_ref())
      .await
  }
}

impl<E, P> Repository<E, P>
where
  E: Entity,
  P: DatabaseProvider,
{
  pub fn query(&self) -> RepositoryQuery<'_, E, P> {
    let builder = if E::is_soft_deletable() {
      QueryBuilder::new().where_is_null("deleted_at")
    } else {
      QueryBuilder::new()
    };
    RepositoryQuery {
      repo: self,
      builder,
    }
  }

  pub fn query_including_deleted(&self) -> RepositoryQuery<'_, E, P> {
    RepositoryQuery {
      repo: self,
      builder: QueryBuilder::new(),
    }
  }
}
