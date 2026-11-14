# Query Builder

`nosql_orm` exposes two query surfaces: the standalone `QueryBuilder` and the
`RepositoryQuery` returned by `Repository::query()` (which wraps a
`QueryBuilder` and binds it to a repository). Both share the same filter model.

---

## Filter

```rust
pub enum Filter {
    Eq(String, Value), Ne(String, Value),
    Gt(String, Value), Gte(String, Value),
    Lt(String, Value), Lte(String, Value),
    In(String, Vec<Value>), NotIn(String, Vec<Value>),
    ArrayContains(String, Value),
    Contains(String, String),          // case-insensitive substring
    StartsWith(String, String),        // case-insensitive
    EndsWith(String, String),         // case-insensitive
    Like(String, String),             // SQL-style % and _
    IsNull(String), IsNotNull(String),
    Between(String, Value, Value),
    And(Vec<Filter>), Or(Vec<Filter>), Not(Box<Filter>),
}
```

`Filter::matches(doc)` evaluates a filter in memory (used by JSON/Redis
providers). `Filter::from_json(value)` parses a Mongo-style descriptor
(`{"field": {"$gt": 5}}`, `{"$and": [...]}`, `{"$or": [...]}`,
`{"$not": ...}`) into a `Filter`.

Helper constructors in `nosql_orm::macros` and the prelude: `eq_filter`,
`ne_filter`, `gt_filter`, `gte_filter`, `lt_filter`, `lte_filter`,
`in_filter`, `and_filter`, `or_filter`, `not_filter`, `is_null_filter`,
`is_not_null_filter`. Macros `filter!` and `filters!` are also exported.

---

## QueryBuilder

```rust
pub struct QueryBuilder {
    filters: Vec<Filter>,
    order: Option<OrderBy>,
    skip: Option<u64>,
    limit: Option<u64>,
    relations: Vec<String>,
    projection: Option<Projection>,
}

impl QueryBuilder {
    pub fn new() -> Self;
    // filters
    pub fn where_eq(self, field, value) -> Self;
    pub fn where_ne(self, field, value) -> Self;
    pub fn where_gt(self, field, value) -> Self;
    pub fn where_gte(self, field, value) -> Self;
    pub fn where_lt(self, field, value) -> Self;
    pub fn where_lte(self, field, value) -> Self;
    pub fn where_in(self, field, values: Vec<Value>) -> Self;
    pub fn where_not_in(self, field, values: Vec<Value>) -> Self;
    pub fn where_contains(self, field, sub: String) -> Self;
    pub fn where_starts_with(self, field, prefix: String) -> Self;
    pub fn where_ends_with(self, field, suffix: String) -> Self;
    pub fn where_like(self, field, pattern: String) -> Self;
    pub fn where_is_null(self, field) -> Self;
    pub fn where_is_not_null(self, field) -> Self;
    pub fn where_between(self, field, min, max) -> Self;
    pub fn filter(self, f: Filter) -> Self;
    pub fn or(self, other: QueryBuilder) -> Self;
    pub fn negate(self) -> Self;
    pub fn where_or(self, field, value) -> Self;
    pub fn where_and(self, field, value) -> Self;
    // combinators
    pub fn combine_and(self, others: Vec<QueryBuilder>) -> Self;
    pub fn combine_or(self, others: Vec<QueryBuilder>) -> Self;
    // ordering / paging
    pub fn order_by(self, order: OrderBy) -> Self;
    pub fn skip(self, n: u64) -> Self;
    pub fn limit(self, n: u64) -> Self;
    // projection
    pub fn select(self, fields: &[&str]) -> Self;
    pub fn exclude(self, fields: &[&str]) -> Self;
    pub fn get_projection(&self) -> Option<&Projection>;
    // relations
    pub fn with_relation(self, name) -> Self;
    // terminal helpers
    pub fn build_filter(&self) -> Option<Filter>;
}
```

`OrderBy { field, direction: SortDirection }` with `OrderBy::asc(field)` /
`OrderBy::desc(field)`. `SortDirection { Asc, Desc }`.

---

## RepositoryQuery (bound to a repository)

Returned by `Repository::query()` and `query_including_deleted()`. Proxies:

`where_eq`, `where_ne`, `where_gt`, `where_lt`, `where_contains`, `where_in`,
`order_by`, `skip`, `limit`, `with_relation`, `filter`, `select`, `exclude`.

Terminal methods:

```rust
pub async fn find(self) -> OrmResult<Vec<E>>;
pub async fn find_one(self) -> OrmResult<Option<E>>;
pub async fn find_with_cursor(self, cursor: Option<Cursor>) -> OrmResult<PaginatedResult<E>>;
pub async fn find_raw(self) -> OrmResult<Vec<Value>>;
pub async fn count(self) -> OrmResult<u64>;
```

`query()` automatically filters out soft-deleted rows (`where_is_null("deleted_at")`)
for soft-deletable entities; use `query_including_deleted()` to include them.

For filters not proxied (e.g. `where_gte`), build a `Filter` and pass it via
`.filter(Filter::Gte(...))`.

---

## Projection

```rust
pub struct Projection { ... }
impl Projection {
    pub fn select(fields: &[&str]) -> Self;
    pub fn exclude(fields: &[&str]) -> Self;
    pub fn apply_recursive(&self, doc: &Value) -> Value;
}
```

Used by `QueryBuilder::select`/`exclude` and `Repository::find_for_frontend`.

---

## Pagination

```rust
pub struct Cursor { last_id, sort_field, sort_asc }
pub struct PaginatedResult<T> { data: Vec<T>, next_cursor: Option<Cursor>, has_more: bool }
```

`RepositoryQuery::find_with_cursor(Some(cursor))` appends the cursor filter and
returns the next cursor.

---

## Query statistics / optimization

`query` module also exposes (mostly advisory, in-memory):

- `QueryStatistics`, `QueryCollectionStats`, `FilterPattern`
- `QueryOptimizer` → `OptimizedQuery`, `OptimizationHint`, `QueryAnalyzer`,
  `QueryHint`
- `IndexRecommender`, `IndexRecommendation`, `QueryStats`
- `ZeroCopyDeserialize`, `DirectDeserializer`, `LazyValue`,
  `TypedArrayDeserializer`

These are advanced helpers; most callers only need `QueryBuilder`.

---

## Examples

### Basic query

```rust
let adults = repo.query()
    .where_gt("age", 18)
    .where_contains("name", "ali")
    .order_by(OrderBy::desc("age"))
    .limit(20)
    .find()
    .await?;
```

### OR / NOT

```rust
let q = repo.query().filter(Filter::Or(vec![
    Filter::Eq("role".into(), "admin".into()),
    Filter::Eq("role".into(), "editor".into()),
]));
```

### Projection

```rust
let rows = repo.query().select(&["id", "name"]).find_raw().await?;
```

### Cursor pagination

```rust
let page = repo.query().order_by(OrderBy::asc("id")).limit(10)
    .find_with_cursor(None).await?;
if page.has_more {
    let next = repo.query().order_by(OrderBy::asc("id")).limit(10)
        .find_with_cursor(page.next_cursor).await?;
}
```

---

## Next steps

- [06-relations.md](06-relations.md) — Relations
- [09-api-reference.md](09-api-reference.md) — API reference