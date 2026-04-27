# API Reference

Quick reference for the main API types and traits.

---

## Core Traits

### Entity

```rust
pub trait Entity: Serialize + DeserializeOwned + Debug + Clone + Send + Sync + 'static {
    fn meta() -> EntityMeta;
    fn fields() -> Vec<FieldMeta> { Vec::new() }
    fn get_id(&self) -> Option<String>;
    fn set_id(&mut self, id: String);
    fn to_value(&self) -> OrmResult<Value>;
    fn from_value(value: Value) -> OrmResult<Self>;
    fn table_name() -> String { Self::meta().table_name }
    fn is_soft_deletable() -> bool { false }
    fn indexes() -> Vec<NosqlIndex> { Vec::new() }
    fn sql_columns() -> Vec<SqlColumnDef> { Vec::new() }
}
```

### WithRelations

```rust
pub trait WithRelations: Entity {
    fn relations() -> Vec<RelationDef> { Vec::new() }
}
```

### SoftDeletable

```rust
pub trait SoftDeletable: Send + Sync {
    fn deleted_at(&self) -> Option<DateTime<Utc>>;
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>);
    fn is_deleted(&self) -> bool { self.deleted_at().is_some() }
    fn mark_deleted(&mut self) { self.set_deleted_at(Some(Utc::now())); }
    fn restore(&mut self) { self.set_deleted_at(None); }
}
```

### Validate

```rust
pub trait Validate {
    fn validate(&self) -> OrmResult<()>;
}
```

### DatabaseProvider

```rust
#[async_trait]
pub trait DatabaseProvider: Send + Sync + Clone + 'static {
    async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value>;
    async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>>;
    async fn find_many(&self, collection: &str, filter: Option<&Filter>, skip: Option<u64>, limit: Option<u64>, sort_by: Option<&str>, sort_asc: bool) -> OrmResult<Vec<Value>>;
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

## Core Structs

### EntityMeta

```rust
pub struct EntityMeta {
    pub table_name: String,
    pub id_field: String,
    pub sql_columns: Option<Vec<SqlColumnDef>>,
}

impl EntityMeta {
    pub fn new(table_name: impl Into<String>) -> Self;
    pub fn with_id_field(mut self, field: impl Into<String>) -> Self;
    pub fn with_sql_columns(mut self, columns: Vec<SqlColumnDef>) -> Self;
    pub fn sql_table_name(&self) -> String;
}
```

### Filter

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

### QueryBuilder

```rust
pub struct QueryBuilder {
    pub filters: Vec<Filter>,
    pub order: Option<OrderBy>,
    pub skip: Option<u64>,
    pub limit: Option<u64>,
    pub relations: Vec<String>,
    pub projection: Option<Projection>,
}

impl QueryBuilder {
    pub fn new() -> Self;
    pub fn where_eq(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self;
    pub fn where_ne(mut self, field: impl Into<String>, value: impl Into<Value>) -> Self;
    // ... many more methods
    pub fn find(self) -> impl Future<Output = OrmResult<Vec<E>>>;
    pub fn find_one(self) -> impl Future<Output = OrmResult<Option<E>>>;
    pub fn count(self) -> impl Future<Output = OrmResult<u64>>;
}
```

### RelationDef

```rust
pub struct RelationDef {
    pub name: String,
    pub relation_type: RelationType,
    pub target_collection: String,
    pub local_key: String,
    pub foreign_key: String,
    pub join_field: Option<String>,
    pub local_key_in_array: Option<String>,
    pub transform_map_via: Option<TransformMapVia>,
    pub on_delete: Option<SqlOnDelete>,
    pub cascade_soft_delete: bool,
    pub cascade_hard_delete: bool,
}

impl RelationDef {
    pub fn many_to_one(name: &str, target: &str, local_key: &str) -> Self;
    pub fn one_to_many(name: &str, target: &str, foreign_key: &str) -> Self;
    pub fn one_to_one(name: &str, target: &str, local_key: &str) -> Self;
    pub fn many_to_many(name: &str, target: &str, join_field: &str) -> Self;
    pub fn on_delete(mut self, action: SqlOnDelete) -> Self;
}
```

### NosqlIndex

```rust
pub struct NosqlIndex { ... }

impl NosqlIndex {
    pub fn single(field: &str, order: i32) -> Self;
    pub fn compound(fields: &[(&str, i32)]) -> Self;
    pub fn unique(field: &str) -> Self;
    pub fn text(fields: &[(&str, i32)]) -> Self;
    pub fn geospatial_2d(field: &str) -> Self;
    pub fn geospatial_2dsphere(field: &str) -> Self;
    pub fn hashed(field: &str) -> Self;
    pub fn ttl(field: &str, seconds: u32) -> Self;
    pub fn name(mut self, name: &str) -> Self;
}
```

---

## Enums

### RelationType

```rust
pub enum RelationType {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
}
```

### SortDirection

```rust
pub enum SortDirection {
    Asc,
    Desc,
}
```

### SqlColumnType

```rust
pub enum SqlColumnType {
    Serial,
    BigSerial,
    Boolean,
    Integer,
    BigInteger,
    SmallInteger,
    Float,
    Double,
    Char(usize),
    VarChar(usize),
    Text,
    Date,
    Time,
    DateTime,
    Timestamp,
    Json,
    JsonB,
    Uuid,
    Array(Box<SqlColumnType>),
}
```

### SqlDialect

```rust
pub enum SqlDialect {
    PostgreSQL,
    MySQL,
    SQLite,
}
```

### SqlOnDelete

```rust
pub enum SqlOnDelete {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}
```

---

## Error Types

### OrmError

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
    Validation(String),
}

pub type OrmResult<T> = Result<T, OrmError>;
```

---

## Providers

```rust
// JSON
pub struct JsonProvider { ... }

// MongoDB  
pub struct MongoProvider { ... }

// Redis
pub struct RedisProvider { ... }

// SQL
#[cfg(feature = "sql-postgres")]
pub struct PostgresProvider { ... }
#[cfg(feature = "sql-sqlite")]
pub struct SqliteProvider { ... }
#[cfg(feature = "sql-mysql")]
pub struct MySqlProvider { ... }
```

---

## Related Documentation

- [01-introduction.md](01-introduction.md) - Introduction
- [02-entity.md](02-entity.md) - Entity system
- [03-provider.md](03-provider.md) - Providers
- [04-repository.md](04-repository.md) - Repository
- [05-query-builder.md](05-query-builder.md) - Query builder
- [06-relations.md](06-relations.md) - Relations
- [07-features/](07-features/) - Features