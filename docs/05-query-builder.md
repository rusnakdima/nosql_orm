# Query Builder

Fluent API for building complex queries.

---

## Table of Contents

1. [QueryBuilder](#querybuilder)
2. [Filters](#filters)
3. [Sorting & Pagination](#sorting--pagination)
4. [Projection](#projection)
5. [Eager Loading](#eager-loading)
6. [Cursor Pagination](#cursor-pagination)

---

## QueryBuilder

### Starting a Query

```rust
let query = repo.query();
let query = repo.query_including_deleted();  // Includes soft-deleted
```

### Building Queries

```rust
let results = repo.query()
    .where_eq("field", value)           // Equal
    .where_ne("field", value)           // Not equal
    .where_gt("field", value)         // Greater than
    .where_gte("field", value)        // Greater or equal
    .where_lt("field", value)         // Less than
    .where_lte("field", value)       // Less or equal
    .where_contains("field", sub)     // Case-insensitive contains
    .where_starts_with("field", prefix)  // Starts with
    .where_ends_with("field", suffix)    // Ends with
    .where_in("field", vec![...])       // IN list
    .where_not_in("field", vec![...])   // NOT IN
    .where_like("field", pattern)      // LIKE pattern
    .where_is_null("field")          // IS NULL
    .where_is_not_null("field")      // IS NOT NULL
    .where_between("field", min, max)  // Between (inclusive)
    .where_and("field", value)       // AND with next condition
    .where_or("field", value)       // OR with next condition
    .where_not("field", value)       // NOT equal
    .order_by(OrderBy)               // Sorting
    .skip(n)                      // Pagination
    .limit(n)                     // Limit
    .select(&["field1", "field2"])  // Select fields
    .exclude(&["field1"])         // Exclude fields
    .with_relation("relations")     // Eager load relations
    .filter(Filter)              // Raw filter
    .find()
    .await?;
```

### Executing Queries

```rust
// Get all results
let results: Vec<User> = query.find().await?;

// Get first result
let user: Option<User> = query.find_one().await?;

// Count results
let count: u64 = query.count().await?;

// Get raw JSON
let docs: Vec<Value> = query.find_raw().await?;

// With cursor
let paginated = query.find_with_cursor(cursor).await?;
```

---

## Filters

### Filter Operators

| Method | Filter | Description |
|--------|-------|-------------|
| `where_eq(field, value)` | `Eq` | Field equals value |
| `where_ne(field, value)` | `Ne` | Field not equal |
| `where_gt(field, value)` | `Gt` | Field > value |
| `where_gte(field, value)` | `Gte` | Field >= value |
| `where_lt(field, value)` | `Lt` | Field < value |
| `where_lte(field, value)` | `Lte` | Field <= value |
| `where_contains(field, sub)` | `Contains` | Case-insensitive substring |
| `where_starts_with(field, prefix)` | `StartsWith` | Starts with prefix |
| `where_ends_with(field, suffix)` | `EndsWith` | Ends with suffix |
| `where_in(field, vec)` | `In` | In list |
| `where_not_in(field, vec)` | `NotIn` | Not in list |
| `where_like(field, pattern)` | `Like` | SQL LIKE pattern |
| `where_is_null(field)` | `IsNull` | IS NULL |
| `where_is_not_null(field)` | `IsNotNull` | IS NOT NULL |
| `where_between(field, min, max)` | `Between` | Between min and max |

### Filter Enum

```rust
pub enum Filter {
    Eq(String, Value),
    Ne(String, Value),
    Gt(String, Value),
    Gte(String, Value),
    Lt(String, Value),
    Lte(String, Value),
    In(String, Vec<Value>),
    NotIn(String, Vec<Value>),
    Contains(String, String),
    StartsWith(String, String),
    EndsWith(String, String),
    Like(String, String),
    IsNull(String),
    IsNotNull(String),
    Between(String, Value, Value),
    And(Vec<Filter>),
    Or(Vec<Filter>),
    Not(Box<Filter>),
}
```

### Complex Filters

#### AND

```rust
// Implicit AND (chained conditions)
repo.query()
    .where_eq("status", "active")
    .where_gt("age", 18)
    .find().await?;

// Explicit AND group
query.and_group(vec![
    QueryBuilder::new().where_eq("status", "active"),
    QueryBuilder::new().where_eq("role", "admin"),
]).find().await?;
```

#### OR

```rust
// Implicit OR (multiple in one call)
repo.query()
    .where_in("status", vec!["active", "pending"])
    .find().await?;

// Explicit OR group
query.or_group(vec![
    QueryBuilder::new().where_eq("status", "active"),
    QueryBuilder::new().where_eq("status", "pending"),
]).find().await?;
```

#### NOT

```rust
repo.query()
    .not()
    .where_eq("status", "deleted")
    .find().await?;
```

#### Nested Groups

```rust
query.filter(Filter::And(vec![
    Filter::Eq("status".into(), "active".into()),
    Filter::Or(vec![
        Filter::Eq("role".into(), "admin".into()),
        Filter::Eq("role".into(), "moderator".into()),
    ]),
])).find().await?;
```

---

## Sorting & Pagination

### OrderBy

```rust
use nosql_orm::query::OrderBy;
use nosql_orm::query::SortDirection;

// Ascending
query.order_by(OrderBy::asc("name"));

// Descending
query.order_by(OrderBy::desc("created_at"));

// With direction
query.order_by(OrderBy {
    field: "name".into(),
    direction: SortDirection::Asc,
});
```

### Pagination

```rust
// Skip and limit
let results = query
    .skip(20)
    .limit(10)
    .find()
    .await?;
```

---

## Projection

Select specific fields or exclude fields.

### Select Fields

```rust
// Only these fields
let results = repo.query()
    .select(&["id", "name", "email"])
    .find()
    .await?;
```

### Exclude Fields

```rust
// Exclude these fields
let results = repo.query()
    .exclude(&["password", "token"])
    .find()
    .await?;
```

### Projection Struct

```rust
pub struct Projection {
    pub select: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}
```

---

## Eager Loading

### Loading Relations

```rust
// Single relation path
let posts = posts.find_with_relations(&["author"]).await?;

// Multiple relation paths
let posts = posts.find_with_relations(&["author", "tags"]).await?;

// Nested relations
let todo = todos.find_with_relations(&["tasks"]).await?;
```

### With Query

```rust
let posts = posts.query_with_relations(
    QueryBuilder::new()
        .where_contains("title", "Rust")
        .limit(10),
    &["author", "tags"]
).await?;
```

---

## Cursor Pagination

More efficient than offset for large datasets.

### Basic Cursor Pagination

```rust
let result = repo.query()
    .order_by(OrderBy::asc("id"))
    .limit(20)
    .find_with_cursor(None)
    .await?;

// First page
for post in result.data {
    println!("{}", post.title);
}

// Next page
let next = repo.query()
    .order_by(OrderBy::asc("id"))
    .limit(20)
    .find_with_cursor(result.next_cursor)
    .await?;
```

### Cursor Struct

```rust
pub struct Cursor {
    pub last_id: String,
    pub sort_field: String,
    pub sort_asc: bool,
}

impl Cursor {
    pub fn new(last_id: String, sort_field: String, sort_asc: bool) -> Self;
    pub fn as_filter(&self) -> Filter;
}
```

### PaginatedResult

```rust
pub struct PaginatedResult<T> {
    pub data: Vec<T>,
    pub next_cursor: Option<Cursor>,
    pub has_more: bool,
}
```

---

## Examples

### Complex Query Example

```rust
let results = repo.query()
    // Filters
    .where_in("status", vec!["active", "pending"])
    .where_gt("age", 18)
    .where_contains("name", "John")
    .where_is_not_null("email")
    
    // Sorting
    .order_by(OrderBy::desc("created_at"))
    
    // Pagination
    .skip(20)
    .limit(10)
    
    // Projection
    .select(&["id", "name", "email"])
    
    // Execute
    .find()
    .await?;
```

### Search with OR

```rust
let results = repo.query()
    .where_or("name", "Alice")
    .where_or("email", "alice@example.com")
    .find()
    .await?;
```

### Query Builder Groups

```rust
let queries = vec![
    QueryBuilder::new().where_eq("role", "admin"),
    QueryBuilder::new().where_eq("role", "moderator"),
];

let results = repo.query()
    .or_group(queries)
    .find()
    .await?;
```

### Query Raw JSON

```rust
let docs = repo.query()
    .where_eq("status", "active")
    .find_raw()
    .await?;

for doc in docs {
    println!("{}", doc);
}
```

---

## Next Steps

- [06-relations.md](06-relations.md) - Relation loading
- [07-features/](07-features/) - Advanced features