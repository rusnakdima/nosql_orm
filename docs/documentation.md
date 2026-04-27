# nosql_orm Documentation

A TypeORM-inspired ORM for NoSQL and SQL databases with a unified, type-safe API in Rust.

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Core Concepts](#core-concepts)
3. [Data Flow](#data-flow)
4. [Module Reference](#module-reference)
5. [Provider System](#provider-system)
6. [Entity System](#entity-system)
7. [Repository Pattern](#repository-pattern)
8. [Query Builder](#query-builder)
9. [Relations](#relations)
10. [Features](#features)
11. [Provider Implementations](#provider-implementations)

---

## Architecture Overview

The library follows a **repository pattern** with a **provider abstraction** that allows swapping between different database backends without changing business logic:

```
┌─────────────────────────────────────────────────────────────────┐
│                      User Code                              │
├─────────────────────────────────────────────────────────────────┤
│  Repository<E, P>          │        RelationRepository      │
├─────────────────────────────────────────────────────────────────┤
│        QueryBuilder        │         Filter               │
├─────────────────────────────────────────────────────────────────┤
│                        Entity                             │
├─────────────────────────────────────────────────────────────────┤
│                    DatabaseProvider                      │
├──────────────┬──────────────┬──────────────┬─────────────┤
│  JsonProvider│ MongoProvider│ RedisProvider│ SQL Providers│
└──────────────┴──────────────┴──────────────┴─────────────┘
```

---

## Core Concepts

### Entity

An `Entity` is any struct that implements the `Entity` trait. It represents a database record:

```rust
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}

impl Entity for User {
    fn meta() -> EntityMeta { EntityMeta::new("users") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}
```

### EntityMeta

Metadata describing an entity's storage:

```rust
pub struct EntityMeta {
    pub table_name: String,      // Collection/table name
    pub id_field: String,      // Primary key field (default: "id")
    pub sql_columns: Option<Vec<SqlColumnDef>>,  // SQL schema
}
```

### DatabaseProvider

The low-level abstraction for database operations. All providers implement:

```rust
#[async_trait]
pub trait DatabaseProvider: Send + Sync + Clone + 'static {
    async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value>;
    async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>>;
    async fn find_many(&self, collection: &str, filter: Option<&Filter>, skip, limit, sort_by, sort_asc) -> OrmResult<Vec<Value>>;
    async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value>;
    async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value>;
    async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool>;
    async fn delete_many(&self, collection: &str, filter: Option<Filter>) -> OrmResult<usize>;
    async fn update_many(&self, collection: &str, filter: Option<Filter>, updates: Value) -> OrmResult<usize>;
    async fn count(&self, collection: &str, filter: Option<&Filter>) -> OrmResult<u64>;
    async fn exists(&self, collection: &str, id: &str) -> OrmResult<bool>;
    async fn create_index(&self, collection: &str, index: &NosqlIndex) -> OrmResult<()>;
    async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()>;
    async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<NosqlIndexInfo>>;
}
```

---

## Data Flow

### 1. CRUD Operations

```
User Code
   │
   ▼
Repository::save(entity)
   │
   ├─► Entity::to_value()     ──► JSON Value
   │                              │
   │◄── Entity::from_value()      ◄─── JSON Value
   │
   ▼
DatabaseProvider::insert(collection, doc)
   │
   ▼
Provider-Specific Implementation (JSON/Mongo/SQL)
```

### 2. Query Operations

```
User Code
   │
   ▼
Repository::query()  ──► RepositoryQuery
   │
   ▼
QueryBuilder (fluent API)
   │
   ├─► .where_eq(field, value) ──► Filter::Eq
   ├─► .where_gt(field, value) ──► Filter::Gt
   ├─► .where_contains(field, sub) ──► Filter::Contains
   ├─► .order_by(OrderBy) ──► sort specification
   ├─► .skip(n) / .limit(n) ──► pagination
   └─► .select(fields) / .exclude(fields) ──► Projection
   │
   ▼
QueryBuilder::build_filter() ──► Option<Filter>
   │
   ▼
DatabaseProvider::find_many(collection, filter, skip, limit, sort_by, sort_asc)
   │
   ▼
Provider Implementation
   │
   ▼
Vec<Value> ──► Entity::from_value() ──► Vec<E>
```

### 3. Relation Loading

```
User Code
   │
   ▼
RelationRepository::find_with_relations(id, ["relation_name"])
   │
   ▼
Repository::find_by_id(id) ──► Option<Entity>
   │
   ▼
RelationLoader::load_relation_recursive(docs, relation_def, &mut visited)
   │
   ├─► Check for circular reference in visited set
   ├─► Load relation via provider
   │     │
   │     ├─► ManyToOne: Query by local_key (e.g., author_id)
   │     ├─► OneToMany: Query by foreign_key (e.g., post.author_id = user.id)
   │     └─► ManyToMany: Resolve via join field array
   │     
   ├─► Auto-load nested relations from target entity
   └─► Group and attach related records to parent
   │
   ▼
WithLoaded<Entity>
```

---

## Module Reference

### Core Modules

| Module | File | Description |
|--------|------|-------------|
| `entity` | `src/entity.rs` | `Entity` trait, `EntityMeta`, `FrontendProjection` |
| `provider` | `src/provider.rs` | `DatabaseProvider` trait, `ProviderConfig` |
| `repository` | `src/repository.rs` | `Repository`, `RelationRepository`, `RepositoryQuery` |
| `query` | `src/query.rs` | `QueryBuilder`, `Filter`, `Projection`, `SortDirection` |
| `relations` | `src/relations.rs` | `RelationDef`, `RelationLoader`, `WithRelations` |
| `field_meta` | `src/field_meta.rs` | `FieldMeta`, `EntityFieldMeta`, field types |
| `error` | `src/error.rs` | `OrmError`, `OrmResult` |

### Provider Modules

| Module | File | Provider |
|--------|------|----------|
| `providers::json` | `src/providers/json.rs` | JSON file storage |
| `providers::mongo` | `src/providers/mongo.rs` | MongoDB |
| `providers::redis` | `src/providers/redis.rs` | Redis |
| `providers::sql` | `src/providers/sql/` | PostgreSQL, SQLite, MySQL |

### Feature Modules

| Module | File | Description |
|--------|------|-------------|
| `validators` | `src/validators/` | Entity validation |
| `soft_delete` | `src/soft_delete.rs` | Soft delete support |
| `timestamps` | `src/timestamps.rs` | Auto timestamps |
| `migrations` | `src/migrations/` | Database migrations |
| `events` | `src/events/` | Entity event listeners |
| `cascade` | `src/cascade.rs` | Cascade delete |
| `lazy` | `src/lazy/` | Lazy loading |
| `embedded` | `src/embedded/` | Embedded entities |
| `inheritance` | `src/inheritance/` | Table inheritance |
| `search` | `src/search/` | Full-text search |
| `aggregation` | `src/aggregation/` | Aggregation pipeline |
| `cdc` | `src/cdc/` | Change data capture |
| `graphql` | `src/graphql/` | GraphQL integration |
| `subscription` | `src/subscription/` | Pub/Sub |
| `cache` | `src/cache/` | Query caching |
| `sql` | `src/sql/` | SQL utilities |
| `constraints` | `src/constraints/` | Schema constraints |
| `logging` | `src/logging/` | Query logging |
| `cli` | `src/cli/` | CLI tools |
| `transaction` | `src/transaction.rs` | Transaction support |

---

## Provider System

### JsonProvider

File-based JSON storage. Each collection is stored as `<collection>.json`:

```rust
let provider = JsonProvider::new("./data").await?;
```

Data flow:
```
insert() ──► In-memory cache (RwLock) ──► Flush to disk (.json)
find_by_id() ──► In-memory cache ──► Load from disk if not cached
update() ──► In-memory cache ──► Mark dirty ──► Flush to disk
```

### MongoProvider

MongoDB driver integration:

```rust
let provider = MongoProvider::connect("mongodb://localhost:27017", "mydb").await?;
```

Mappings:
- `id` → MongoDB `_id`
- `Filter` → MongoDB query documents
- `NosqlIndex` → MongoDB indexes

### RedisProvider

Key-value storage, caching, pub/sub:

```rust
let provider = RedisProvider::connect("redis://localhost:6379").await?;
```

### SQL Providers

PostgreSQL, SQLite, MySQL via `src/providers/sql/`:

```rust
#[cfg(feature = "sql-postgres")]
let provider = PostgresProvider::connect("postgres://user:pass@localhost/db").await?;

#[cfg(feature = "sql-sqlite")]
let provider = SqliteProvider::connect("app.db").await?;

#[cfg(feature = "sql-mysql")]
let provider = MySqlProvider::connect("mysql://user:pass@localhost/db").await?;
```

---

## Entity System

### Entity Trait

```rust
pub trait Entity: Serialize + DeserializeOwned + Debug + Clone + Send + Sync + 'static {
    fn meta() -> EntityMeta;
    fn fields() -> Vec<FieldMeta> { Vec::new() }
    fn get_id(&self) -> Option<String>;
    fn set_id(&mut self, id: String);
    fn to_value(&self) -> OrmResult<Value>;
    fn from_value(value: Value) -> OrmResult<Self>;
    fn table_name() -> String;
    fn is_soft_deletable() -> bool { false }
    fn indexes() -> Vec<NosqlIndex> { Vec::new() }
    fn sql_columns() -> Vec<SqlColumnDef> { Vec::new() }
}
```

### WithRelations Trait

```rust
pub trait WithRelations: Entity {
    fn relations() -> Vec<RelationDef> { Vec::new() }
}
```

### SoftDeletable Trait

```rust
pub trait SoftDeletable: Send + Sync {
    fn deleted_at(&self) -> Option<DateTime<Utc>>;
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>);
    fn is_deleted(&self) -> bool;
    fn mark_deleted(&mut self);
    fn restore(&mut self);
}
```

### Validate Trait

```rust
pub trait Validate {
    fn validate(&self) -> OrmResult<()>;
}
```

---

## Repository Pattern

### Repository

Generic CRUD operations:

```rust
pub struct Repository<E, P> where E: Entity, P: DatabaseProvider {
    provider: P,
    _phantom: PhantomData<E>,
}
```

Methods:
- `insert(entity)` - Insert new record
- `update(entity)` - Update existing record  
- `save(entity)` - Insert or update
- `find_by_id(id)` - Find by primary key
- `get_by_id(id)` - Find or error
- `find_all()` - Get all records
- `delete(id)` - Delete by id
- `remove(entity)` - Delete entity
- `patch(id, patch)` - Partial update
- `soft_delete(id)` - Soft delete
- `restore(id)` - Restore soft-deleted
- `count()` - Count records
- `exists(id)` - Check existence
- `query()` - Start fluent query

### RelationRepository

Extends Repository with relation loading:

```rust
pub struct RelationRepository<E, P> where E: WithRelations, P: DatabaseProvider {
    inner: Repository<E, P>,
    loader: RelationLoader<P>,
}
```

Methods:
- `find_with_relations(id, paths)` - Load entity with relations
- `find_all_with_relations(paths)` - Load all with relations
- `query_with_relations(builder, paths)` - Query with relations

---

## Query Builder

### QueryBuilder

Fluent query construction:

```rust
repo.query()
    .where_eq("field", value)       // Exact match
    .where_ne("field", value)       // Not equal
    .where_gt("field", value)       // Greater than
    .where_lt("field", value)       // Less than
    .where_gte("field", value)      // Greater or equal
    .where_lte("field", value)      // Less or equal
    .where_contains("field", sub)    // Case-insensitive contains
    .where_starts_with("field", prefix)
    .where_ends_with("field", suffix)
    .where_in("field", values)       // IN list
    .where_not_in("field", values)  // NOT IN
    .where_like("field", pattern) // LIKE pattern
    .where_is_null("field")        // IS NULL
    .where_is_not_null("field")     // IS NOT NULL
    .where_between("field", min, max)
    .order_by(OrderBy)            // Sorting
    .skip(n)                      // Pagination
    .limit(n)                     // Limit
    .select(&["field1", "field2"]) // Field projection (include)
    .exclude(&["field1"])          // Field projection (exclude)
    .with_relation("rel_name")     // Eager relation
    .filter(Filter)               // Raw filter
    .find()                      // Execute
    .find_one()                   // First result
    .count()                     // Count
    .find_raw()                   // Raw JSON
    .find_with_cursor(cursor)      // Cursor pagination
```

### Filter

Composable filter conditions:

```rust
pub enum Filter {
    Eq(String, Value),           // Equal
    Ne(String, Value),          // Not equal
    Gt(String, Value),         // Greater than
    Gte(String, Value),        // Greater or equal
    Lt(String, Value),          // Less than
    Lte(String, Value),         // Less or equal
    In(String, Vec<Value>),     // IN list
    NotIn(String, Vec<Value>),    // NOT IN
    Contains(String, String),   // Case-insensitive contains
    StartsWith(String, String),   // Starts with
    EndsWith(String, String),    // Ends with
    Like(String, String),       // LIKE pattern
    IsNull(String),           // IS NULL
    IsNotNull(String),        // IS NOT NULL
    Between(String, Value, Value),
    And(Vec<Filter>),         // AND group
    Or(Vec<Filter>),          // OR group
    Not(Box<Filter>),        // NOT wrapper
}
```

### Projection

Field selection/exclusion:

```rust
pub struct Projection {
    select: Option<Vec<String>>,  // Fields to include
    exclude: Option<Vec<String>>,   // Fields to exclude
}
```

---

## Relations

### RelationDef

Defines relationships between entities:

```rust
pub struct RelationDef {
    pub name: String,              // Relation name (e.g., "author")
    pub relation_type: RelationType,
    pub target_collection: String,    // Related collection
    pub local_key: String,          // FK on this side
    pub foreign_key: String,       // FK on target side
    pub join_field: Option<String>, // For ManyToMany
    pub local_key_in_array: Option<String>,
    pub transform_map_via: Option<TransformMapVia>,
    pub on_delete: Option<SqlOnDelete>,
    pub cascade_soft_delete: bool,
    pub cascade_hard_delete: bool,
}
```

### RelationTypes

```rust
pub enum RelationType {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
}
```

### RelationLoader

Batch loads relations efficiently:

```rust
pub struct RelationLoader<P: DatabaseProvider> {
    provider: P,
}
```

Key features:
- Batch loading (avoids N+1 queries)
- Circular reference detection
- Nested relation auto-loading
- Soft delete filtering
- Transform via mapping

### WithLoaded

Entity with loaded relations:

```rust
pub struct WithLoaded<E: Entity> {
    pub entity: E,
    pub loaded: HashMap<String, RelationValue>,
}
```

Methods:
- `one("relation")` - Get single relation
- `many("relation")` - Get collection relation
- `has("relation")` - Check if loaded

---

## Features

### Validators

```rust
pub enum ValidatorType {
    Email,
    NotEmpty,
    NonNull,
    Length,
    Pattern,
    Range,
    Uuid,
    Url,
    Min,
    Max,
    Required,
}
```

### Migrations

```rust
pub struct MigrationRunner<P: DatabaseProvider> {
    provider: P,
    migrations: Vec<Box<dyn Migration<P>>>,
}
```

Methods:
- `add_migration(migration)`
- `run_all_pending()` - Run pending migrations
- `rollback(count)` - Rollback migrations
- `status()` - Migration status

### Indexes

```rust
pub enum NosqlIndexType {
    Single,
    Compound,
    Unique,
    TTL,
    Text,
    Geospatial2d,
    Geospatial2dsphere,
    Hashed,
}
```

### Timestamps

Auto `created_at` and `updated_at`:

```rust
apply_timestamps(&mut doc, is_insert);
```

### Cascade Delete

```rust
pub struct CascadeManager<P: DatabaseProvider> {
    provider: P,
}
```

### Aggregations

```rust
pub enum Stage {
    Match(Filter),
    Sort(OrderBy),
    Skip(u64),
    Limit(u64),
    Project(Projection),
    Group { _key: String, _accumulators: Vec<Accumulator> },
}
```

---

## Provider Implementations

### Feature Flags

```toml
[features]
default = ["json"]
json = []
mongo = ["dep:mongodb", "dep:futures-util"]
redis = ["dep:redis"]
full = ["json", "mongo", "redis"]
query_cache = []
validators = []

# SQL Providers
sql-postgres = ["dep:tokio-postgres", "dep:deadpool-postgres", "dep:base64"]
sql-sqlite = ["dep:rusqlite", "dep:base64"]
sql-mysql = ["dep:mysql_async", "dep:base64"]
sql = ["sql-postgres", "sql-sqlite", "sql-mysql"]
```

### Creating a Provider

```rust
// JSON (embedded, zero-config)
let provider = JsonProvider::new("./data").await?;

// MongoDB
let provider = MongoProvider::connect("mongodb://localhost:27017", "mydb").await?;

// From config
let config = ProviderConfig::new("mongodb://localhost:27017")
    .with_database("mydb");
let provider = MongoProvider::from_config(&config).await?;

// SQL
#[cfg(feature = "sql-postgres")]
let provider = PostgresProvider::connect("postgres://user:pass@localhost/db").await?;

#[cfg(feature = "sql-sqlite")]
let provider = SqliteProvider::connect("app.db").await?;
```

---

## Complete Example

```rust
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}

impl Entity for User {
    fn meta() -> EntityMeta { EntityMeta::new("users") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub author_id: String,
    pub tag_ids: Vec<String>,
}

impl Entity for Post {
    fn meta() -> EntityMeta { EntityMeta::new("posts") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for Post {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::many_to_one("author", "users", "author_id"),
            RelationDef::many_to_many("tags", "tags", "tag_ids"),
        ]
    }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    // Create provider
    let provider = JsonProvider::new("./data").await?;
    
    // Create repositories
    let users = Repository::<User>::new(provider.clone());
    let posts = RelationRepository::<Post, _>::new(provider.clone());
    
    // Register relations
    register_relations_for_entity::<Post>();
    
    // CRUD
    let user = users.save(User {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).await?;
    
    let post = posts.save(Post {
        id: None,
        title: "Hello World".into(),
        author_id: user.id.clone().unwrap(),
        tag_ids: vec![],
    }).await?;
    
    // Query
    let results = posts.query()
        .where_contains("title", "Hello")
        .order_by(OrderBy::desc("id"))
        .limit(10)
        .find()
        .await?;
    
    // Load with relations
    let with_author = posts
        .find_with_relations(&post.id.unwrap(), &["author"])
        .await?
        .unwrap();
    
    if let Some(author) = with_author.one("author")? {
        println!("Author: {}", author["name"]);
    }
    
    Ok(())
}
```

---

## Error Handling

```rust
pub enum OrmError {
    NotFound(String),
    Duplicate(String),
    Serialization(serde_json::Error),
    Io(std::io::Error),
    Provider(String),
    Relation(String),
    InvalidQuery(String),
    InvalidInput(String),
    Query(String),
    Connection(String),
    Transaction(String),
    CascadeRestricted { entity: String, relation: String },
    #[cfg(feature = "mongo")]
    Mongo(mongodb::error::Error),
    #[cfg(feature = "redis")]
    Redis(redis::RedisError),
    Validation(String),
}

pub type OrmResult<T> = Result<T, OrmError>;
```

---

## Macro Support

### Model Macro

```rust
#[derive(Model, Serialize, Deserialize)]
#[table_name("users")]
#[id_field("id")]
pub struct User {
    pub id: Option<String>,
    pub name: String,
}
```

### Validate Macro

```rust
#[derive(Validate, Serialize, Deserialize)]
pub struct User {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 2, max = 50))]
    pub name: String,
}
```

---

## Additional Resources

- [README.md](../README.md) - Quick start guide
- [plan.md](../plan.md) - Feature roadmap
- Examples in `examples/` directory