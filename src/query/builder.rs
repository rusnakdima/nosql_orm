use serde_json::Value;

use super::filter::Filter;
use super::projection::Projection;
use super::types::OrderBy;

#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
  pub(crate) filters: Vec<Filter>,
  pub(crate) order: Option<OrderBy>,
  pub(crate) skip: Option<u64>,
  pub(crate) limit: Option<u64>,
  pub(crate) relations: Vec<String>,
  pub(crate) projection: Option<Projection>,
}

impl QueryBuilder {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn where_eq(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Eq(field.into(), value.into()));
    self
  }

  pub fn where_ne(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Ne(field.into(), value.into()));
    self
  }

  pub fn where_gt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Gt(field.into(), value.into()));
    self
  }

  pub fn where_lt(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Lt(field.into(), value.into()));
    self
  }

  pub fn where_contains(mut self, field: impl Into<String>, sub: impl Into<String>) -> Self {
    self
      .filters
      .push(Filter::Contains(field.into(), sub.into()));
    self
  }

  pub fn where_starts_with(mut self, field: impl Into<String>, prefix: impl Into<String>) -> Self {
    self
      .filters
      .push(Filter::StartsWith(field.into(), prefix.into()));
    self
  }

  pub fn where_in(mut self, field: impl Into<String>, values: Vec<Value>) -> Self {
    self.filters.push(Filter::In(field.into(), values));
    self
  }

  pub fn filter(mut self, f: Filter) -> Self {
    self.filters.push(f);
    self
  }

  pub fn where_is_null(mut self, field: impl Into<String>) -> Self {
    self.filters.push(Filter::IsNull(field.into()));
    self
  }

  pub fn order_by(mut self, order: OrderBy) -> Self {
    self.order = Some(order);
    self
  }

  pub fn skip(mut self, n: u64) -> Self {
    self.skip = Some(n);
    self
  }

  pub fn limit(mut self, n: u64) -> Self {
    self.limit = Some(n);
    self
  }

  pub fn with_relation(mut self, name: impl Into<String>) -> Self {
    self.relations.push(name.into());
    self
  }

  pub fn select(mut self, fields: &[&str]) -> Self {
    self.projection = Some(Projection::select(fields));
    self
  }

  pub fn exclude(mut self, fields: &[&str]) -> Self {
    self.projection = Some(Projection::exclude(fields));
    self
  }

  pub fn where_gte(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Gte(field.into(), value.into()));
    self
  }

  pub fn where_lte(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Lte(field.into(), value.into()));
    self
  }

  pub fn where_not_in(mut self, field: impl Into<String>, values: Vec<Value>) -> Self {
    self.filters.push(Filter::NotIn(field.into(), values));
    self
  }

  pub fn where_ends_with(mut self, field: impl Into<String>, suffix: impl Into<String>) -> Self {
    self
      .filters
      .push(Filter::EndsWith(field.into(), suffix.into()));
    self
  }

  pub fn where_like(mut self, field: impl Into<String>, pattern: impl Into<String>) -> Self {
    self
      .filters
      .push(Filter::Like(field.into(), pattern.into()));
    self
  }

  pub fn where_is_not_null(mut self, field: impl Into<String>) -> Self {
    self.filters.push(Filter::IsNotNull(field.into()));
    self
  }

  pub fn where_between(
    mut self,
    field: impl Into<String>,
    min: impl Into<Value>,
    max: impl Into<Value>,
  ) -> Self {
    self
      .filters
      .push(Filter::Between(field.into(), min.into(), max.into()));
    self
  }

  pub fn or(mut self, other: QueryBuilder) -> Self {
    let combined = vec![
      self.build_filter().unwrap_or(Filter::And(vec![])),
      other.build_filter().unwrap_or(Filter::And(vec![])),
    ];
    self.filters = vec![Filter::Or(combined)];
    self
  }

  pub fn negate(mut self) -> Self {
    if let Some(f) = self.build_filter() {
      self.filters = vec![Filter::Not(Box::new(f))];
    }
    self
  }

  pub fn where_or(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    let filter = Filter::Eq(field.into(), value.into());
    self.filters.push(Filter::Or(vec![filter]));
    self
  }

  pub fn where_and(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    let filter = Filter::Eq(field.into(), value.into());
    self.filters.push(filter);
    self
  }

  pub fn where_not(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self {
    self.filters.push(Filter::Not(Box::new(Filter::Eq(
      field.into(),
      value.into(),
    ))));
    self
  }

  pub fn or_group(mut self, others: Vec<QueryBuilder>) -> Self {
    let mut all_filters = Vec::new();
    if let Some(f) = self.build_filter() {
      all_filters.push(f);
    }
    for builder in others {
      if let Some(f) = builder.build_filter() {
        all_filters.push(f);
      }
    }
    self.filters = vec![Filter::Or(all_filters)];
    self
  }

  pub fn and_group(mut self, others: Vec<QueryBuilder>) -> Self {
    let mut all_filters = Vec::new();
    if let Some(f) = self.build_filter() {
      all_filters.push(f);
    }
    for builder in others {
      if let Some(f) = builder.build_filter() {
        all_filters.push(f);
      }
    }
    self.filters = vec![Filter::And(all_filters)];
    self
  }

  pub fn get_projection(&self) -> Option<&Projection> {
    self.projection.as_ref()
  }

  pub fn build_filter(&self) -> Option<Filter> {
    match self.filters.len() {
      0 => None,
      1 => Some(self.filters[0].clone()),
      _ => Some(Filter::And(self.filters.clone())),
    }
  }

  pub fn get_cursor(&self) -> Option<String> {
    None
  }
}
