# Entity System

The `Entity` trait is the core abstraction representing a database record. This document covers implementing entities manually and using macros.

---

## Table of Contents

1. [Basic Entity Implementation](#basic-entity-implementation)
2. [EntityMeta](#entitymeta)
3. [Entity Traits](#entity-traits)
4. [Using Macros](#using-macros)
5. [Field Types](#field-types)
6. [SQL Columns](#sql-columns)

---

## Basic Entity Implementation

### Simple Entity

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
    fn meta() -> EntityMeta {
        EntityMeta::new("users")
    }

    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}
```

### Entity with Relations

```rust
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub author_id: String,
    pub tag_ids: Vec<String>,
}

impl Entity for Post {
    fn meta() -> EntityMeta {
        EntityMeta::new("posts")
    }

    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}

impl WithRelations for Post {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::many_to_one("author", "users", "author_id"),
            RelationDef::many_to_many("tags", "tags", "tag_ids"),
        ]
    }
}
```

### Entity with Soft Delete

```rust
use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm::SoftDeletable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftDeletableUser {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for SoftDeletableUser {
    fn meta() -> EntityMeta {
        EntityMeta::new("soft_deletable_users")
    }

    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }

    fn is_soft_deletable() -> bool {
        true
    }
}

impl SoftDeletable for SoftDeletableUser {
    fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }

    fn set_deleted_at(&mut self, deleted_at:Option<DateTime<Utc>>) {
        self.deleted_at = deleted_at;
    }
}
```

---

## EntityMeta

Metadata describing an entity's storage.

### Creating EntityMeta

```rust
let meta = EntityMeta::new("users");

// With custom id field
let meta = EntityMeta::new("users").with_id_field("user_id");

// With SQL columns
let meta = EntityMeta::new("users").with_sql_columns(vec![
    SqlColumnDef::new("id", SqlColumnType::Serial).primary_key(),
    SqlColumnDef::new("name", SqlColumnType::VarChar(255)),
]);
```

### EntityMeta Fields

```rust
pub struct EntityMeta {
    pub table_name: String,      // Collection/table name
    pub id_field: String,       // Primary key field (default: "id")
    pub sql_columns: Option<Vec<SqlColumnDef>>,  // SQL schema
}
```

### EntityMeta Methods

```rust
impl EntityMeta {
    pub fn new(table_name: impl Into<String>) -> Self;
    pub fn with_id_field(mut self, field: impl Into<String>) -> Self;
    pub fn with_sql_columns(mut self, columns: Vec<SqlColumnDef>) -> Self;
    pub fn sql_table_name(&self) -> String;
}
```

---

## Entity Traits

### Entity Trait

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
    fn is_deleted(&self) -> bool {
        self.deleted_at().is_some()
    }
    fn mark_deleted(&mut self) {
        self.set_deleted_at(Some(Utc::now()));
    }
    fn restore(&mut self) {
        self.set_deleted_at(None);
    }
}
```

### Validate Trait

```rust
pub trait Validate {
    fn validate(&self) -> OrmResult<()>;
}

impl<T: Validate> Validate for Option<T> { ... }
impl<T: Validate> Validate for Vec<T> { ... }
```

---

## Using Macros

### Model Macro

The `#[derive(Model)]` macro automatically implements `Entity` and optionally `WithRelations`:

```rust
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("users")]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

### Macro Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `#[table_name("...")]` | Set collection name | `#[table_name("users")]` |
| `#[id_field("...")]` | Set id field name | `#[id_field("user_id")]` |
| `#[soft_delete]` | Enable soft delete | `#[soft_delete]` |
| `#[timestamp]` | Auto timestamps | `#[timestamp]` |
| `#[one_to_many(...)]` | Define 1:N relation | `#[one_to_many("posts", "posts", "user_id")]` |
| `#[many_to_one(...)]` | Define N:1 relation | `#[many_to_one("author", "users", "author_id")]`` |
| `#[one_to_one(...)]` | Define 1:1 relation | `#[one_to_one("profile", "profiles", "profile_id")]`` |
| `#[many_to_many(...)]` | Define N:M relation | `#[many_to_many("tags", "tags", "tag_ids")]` |

### Validate Macro

```rust
use nosql_orm_derive::Validate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct User {
    #[validate(email)]
    pub email: String,
    
    #[validate(length(min = 2, max = 50))]
    pub name: String,
}
```

### Validation Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `#[validate(email)]` | Valid email format | `#[validate(email)]` |
| `#[validate(uuid)]` | Valid UUID format | `#[validate(uuid)]` |
| `#[validate(url)]` | Valid URL format | `#[validate(url)]` |
| `#[validate(not_empty)]` | Not empty | `#[validate(not_empty)]` |
| `#[validate(non_null)]` | Not null | `#[validate(non_null)]` |
| `#[validate(required)]` | Required field | `#[validate(required)]` |
| `#[validate(length(min = N, max = M))` | Length bounds | `#[validate(length(min = 2, max = 50))]` |
| `#[validate(min = N)]` | Minimum value | `#[validate(min = 18)]` |
| `#[validate(max = N)]` | Maximum value | `#[validate(max = 150)]` |
| `#[validate(range(min = N, max = M))` | Value range | `#[validate(range(min = 0, max = 100))]` |
| `#[validate(pattern("regex"))` | Regex pattern | `#[validate(pattern(r"^[a-z]+$")]` |

---

## Field Types

### FieldType Enum

```rust
pub enum FieldType {
    Id,
    Column,
    Relation(RelationFieldType),
}
```

### RelationFieldType

```rust
pub enum RelationFieldType {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
    ManyToOneArray,
}
```

### FieldMeta

```rust
pub struct FieldMeta {
    pub name: String,
    pub field_type: FieldType,
    pub relation: Option<RelationMeta>,
    pub validators: Vec<ValidateMeta>,
    pub is_optional: bool,
    pub is_timestamp: bool,
    pub is_soft_delete: bool,
}
```

---

## SQL Columns

### Defining SQL Schema

```rust
impl Entity for User {
    fn sql_columns() -> Vec<SqlColumnDef> {
        vec![
            SqlColumnDef::new("id", SqlColumnType::Serial).primary_key(),
            SqlColumnDef::new("name", SqlColumnType::VarChar(255)),
            SqlColumnDef::new("email", SqlColumnType::VarChar(255)).unique(),
            SqlColumnDef::new("age", SqlColumnType::Integer),
        ]
    }
}
```

### SqlColumnDef

```rust
pub struct SqlColumnDef {
    pub name: String,
    pub column_type: SqlColumnType,
    pub primary_key: bool,
    pub unique: bool,
    pub nullable: bool,
    pub default: Option<String>,
    pub check: Option<String>,
    pub references: Option<(String, String)>,  // (table, column)
}

impl SqlColumnDef {
    pub fn new(name: impl Into<String>, column_type: SqlColumnType) -> Self;
    pub fn primary_key(mut self) -> Self;
    pub fn unique(mut self) -> Self;
    pub fn nullable(mut self) -> Self;
    pub fn default(mut self, value: impl Into<String>) -> Self;
    pub fn check(mut self, condition: impl Into<String>) -> Self;
    pub fn references(mut self, table: impl Into<String>, column: impl Into<String>) -> Self;
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

---

## Indexes

### Defining Indexes

```rust
impl Entity for User {
    fn indexes() -> Vec<NosqlIndex> {
        vec![
            NosqlIndex::single("email", 1).unique(true),
            NosqlIndex::compound(&[("last_name", 1), ("first_name", 1)]),
            NosqlIndex::text(&[("name", 10), ("email", 5)]),
            NosqlIndex::ttl("created_at", 3600),
        ]
    }
}
```

---

## Examples

### Complete Entity Example

```rust
use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm::SoftDeletable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub age: u32,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for User {
    fn meta() -> EntityMeta {
        EntityMeta::new("users")
    }

    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }

    fn is_soft_deletable() -> bool {
        true
    }

    fn indexes() -> Vec<NosqlIndex> {
        vec![
            NosqlIndex::single("email", 1).unique(true),
        ]
    }

    fn sql_columns() -> Vec<SqlColumnDef> {
        vec![
            SqlColumnDef::new("id", SqlColumnType::Serial).primary_key(),
            SqlColumnDef::new("name", SqlColumnType::VarChar(255)),
            SqlColumnDef::new("email", SqlColumnType::VarChar(255)).unique(),
            SqlColumnDef::new("age", SqlColumnType::Integer),
        ]
    }
}

impl WithRelations for User {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::one_to_many("posts", "posts", "author_id"),
        ]
    }
}

impl SoftDeletable for User {
    fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }

    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
        self.deleted_at = deleted_at;
    }
}
```

### Using Macro (Simplified)

```rust
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("users")]
#[soft_delete]
#[timestamp]
#[index("email", 1, "unique")]
#[sql_column("id", "serial", "primary")]
#[sql_column("name", "varchar", "255")]
#[sql_column("email", "varchar", "255", "unique")]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

This auto-generates:
- `Entity` implementation with table name
- `SoftDeletable` tracking (if `#[soft_delete]`)
- `indexes()` method returning defined indexes
- `sql_columns()` method returning SQL column definitions

### Using Macro with Relations

```rust
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("posts")]
#[many_to_one("author", "users", "author_id")]
#[many_to_many("tags", "tags", "tag_ids")]
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub author_id: String,
    pub tag_ids: Vec<String>,
}
```

---

## Next Steps

- [03-provider.md](03-provider.md) - Database providers
- [04-repository.md](04-repository.md) - Repository CRUD
- [07-features/07a-macros.md](07-features/07a-macros.md) - Detailed macro documentation
- [07-features/07b-validators.md](07-features/07b-validators.md) - Validators
- [07-features/07c-soft-delete.md](07-features/07c-soft-delete.md) - Soft deletes