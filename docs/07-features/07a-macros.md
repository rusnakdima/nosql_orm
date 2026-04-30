# Macros

Derive macros for automatic Entity, Model, OrmEntity, and Validate trait implementation.

---

## Table of Contents

1. [OrmEntity Macro](#ormentity-macro) (Recommended)
2. [Model Macro](#model-macro)
3. [Entity Macro](#entity-macro)
4. [Validate Macro](#validate-macro)
5. [Index Attributes](#index-attributes)
6. [SQL Column Attributes](#sql-column-attributes)
7. [Relation Attributes](#relation-attributes)
8. [Complete Examples](#complete-examples)

---

## OrmEntity Macro (Recommended)

The `#[derive(OrmEntity)]` macro is the recommended way to define entities. It automatically adds `Serialize`, `Deserialize`, `Debug`, and `Clone` derives, making entity definition minimal.

### Basic Usage

```rust
use nosql_orm_derive::OrmEntity;

#[derive(OrmEntity)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

This automatically generates:
- `#[derive(Serialize, Deserialize, Debug, Clone)]`
- `impl Entity for User { ... }`

### With Options

```rust
use nosql_orm_derive::OrmEntity;

#[derive(OrmEntity)]
#[table_name("users")]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

### Available Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `#[table_name("name")]` | Set collection/table name | `#[table_name("users")]` |
| `#[id_field("name")]` | Set id field name | `#[id_field("user_id")]` |
| `#[soft_delete]` | Enable soft delete | `#[soft_delete]` |
| `#[timestamp]` | Auto create timestamps | `#[timestamp]` |
| `#[Relations(...)]` | Simple relation syntax | `#[Relations(posts, comments)]` |
| `#[index(...)]` | Define indexes | `#[index("email", 1, "unique")]` |
| `#[sql_column(...)]` | Define SQL columns | `#[sql_column("id", "serial", "primary")]` |
| `#[frontend_exclude(...)]` | Fields hidden from frontend | `#[frontend_exclude("password")]` |

---

## Model Macro

The `#[derive(Model)]` macro is an alias for `Entity` with a friendlier name.

### Basic Usage

```rust
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

---

## Entity Macro

The `#[derive(Entity)]` macro provides the core `Entity` trait implementation.

### Basic Usage

```rust
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

### Available Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `#[table_name("name")]` | Set collection/table name | `#[table_name("users")]` |
| `#[id_field("name")]` | Set id field name | `#[id_field("user_id")]` |
| `#[soft_delete]` | Enable soft delete | `#[soft_delete]` |
| `#[timestamp]` | Auto create timestamps | `#[timestamp]` |

### Relation Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `#[one_to_many(...)]` | Define 1:N relation | `#[one_to_many("posts", "posts", "user_id")]` |
| `#[many_to_one(...)]` | Define N:1 relation | `#[many_to_one("author", "users", "author_id")]` |
| `#[one_to_one(...)]` | Define 1:1 relation | `#[one_to_one("profile", "profiles", "profile_id")]` |
| `#[many_to_many(...)]` | Define N:M relation | `#[many_to_many("tags", "tags", "tag_ids")]` |

### Index Attributes

Define indexes using `#[index(...)]` or `#[index = "..."]`:

| Format | Description |
|--------|-------------|
| `#[index("field")]` | Single field index |
| `#[index("field", 1)]` | Single field with order (1=asc, -1=desc) |
| `#[index("field", 1, "unique")]` | Unique index |
| `#[index = "field"]` | Alternative syntax |

### SQL Column Attributes

Define SQL columns using `#[sql_column(...)]` or `#[sql_column = "..."]`:

| Format | Description |
|--------|-------------|
| `#[sql_column("name", "type")]` | Column with type |
| `#[sql_column("name", "type", "unique")]` | Unique column |
| `#[sql_column("name", "type", "primary")]` | Primary key |
| `#[sql_column = "name,type"]` | Alternative syntax |

### Available SQL Types

| Type | Description |
|------|-------------|
| `serial` | Auto-increment integer |
| `bigserial` | Large auto-increment |
| `boolean` | True/false |
| `integer` | 32-bit integer |
| `bigint` | 64-bit integer |
| `smallint` | 16-bit integer |
| `float` | 32-bit float |
| `double` | 64-bit float |
| `varchar` | Variable length (needs size) |
| `char` | Fixed length |
| `text` | Unlimited text |
| `date` | Date |
| `time` | Time |
| `datetime` | DateTime |
| `timestamp` | Timestamp |
| `json` | JSON |
| `jsonb` | JSON (binary) |
| `uuid` | UUID |

### Example: Simple Entity

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("users")]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

Generates:

```rust
impl Entity for User {
    fn meta() -> EntityMeta {
        EntityMeta::new("users")
    }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}
```

### Example: With Soft Delete

```rust
use nosql_orm_derive::OrmEntity;

#[derive(OrmEntity)]
#[table_name("users")]
#[soft_delete]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub deleted_at: Option<DateTime<Utc>>,
}
```

### Example: With Timestamps

```rust
use nosql_orm_derive::OrmEntity;

#[derive(OrmEntity)]
#[table_name("users")]
#[timestamp]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

### Example: With Custom ID

```rust
use nosql_orm_derive::OrmEntity;

#[derive(OrmEntity)]
#[table_name("users")]
#[id_field("user_id")]
pub struct User {
    pub id: Option<String>,  // Maps to id_field = "user_id"
    pub name: String,
}
```

---

## Validate Macro

The `#[derive(Validate)]` macro generates validation code.

### Basic Usage

```rust
use nosql_orm_derive::{OrmEntity, Validate};

#[derive(OrmEntity, Validate)]
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
| `#[validate(email)]` | Valid email | `#[validate(email)]` |
| `#[validate(uuid)]` | Valid UUID | `#[validate(uuid)]` |
| `#[validate(url)]` | Valid URL | `#[validate(url)]` |
| `#[validate(not_empty)]` | Not empty string | `#[validate(not_empty)]` |
| `#[validate(non_null)]` | Not null | `#[validate(non_null)]` |
| `#[validate(required)]` | Required | `#[validate(required)]` |
| `#[validate(length(min = N, max = M))]` | String length | `#[validate(length(min = 2, max = 50))]` |
| `#[validate(min = N)]` | Minimum | `#[validate(min = 18)]` |
| `#[validate(max = N)]` | Maximum | `#[validate(max = 150)]` |
| `#[validate(range(min = N, max = M))]` | Range | `#[validate(range(min = 0, max = 100))]` |
| `#[validate(pattern("regex"))]` | Regex | `#[validate(pattern(r"^[a-z]+$"))]` |

### Using Validate

```rust
use nosql_orm_derive::{OrmEntity, Validate};

#[derive(OrmEntity, Validate)]
pub struct User {
    #[validate(email)]
    pub email: String,

    #[validate(length(min = 2, max = 50))]
    pub name: String,

    #[validate(min = 18)]
    pub age: u32,
}

fn main() {
    let user = User {
        id: None,
        email: "invalid".to_string(),
        name: "A".to_string(),
        age: 15,
    };

    if let Err(e) = user.validate() {
        println!("Validation failed: {}", e);
    }
}
```

---

## Complete Examples

### Example: Entity with Relations (OrmEntity)

```rust
use nosql_orm_derive::OrmEntity;

#[derive(OrmEntity)]
#[table_name("posts")]
#[Relations(authors, categories)]
#[soft_delete]
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub author_id: String,
    pub category_ids: Vec<String>,
    pub deleted_at: Option<DateTime<Utc>>,
}
```

### Example: Full Stack with OrmEntity

```rust
use nosql_orm::prelude::*;
use nosql_orm_derive::OrmEntity;
use nosql_orm::Validate;

#[derive(OrmEntity, Validate)]
#[table_name("users")]
#[soft_delete]
#[timestamp]
#[index("email", 1, "unique")]
#[sql_column("id", "serial", "primary")]
#[sql_column("name", "varchar", "255")]
#[sql_column("email", "varchar", "255", "unique")]
#[sql_column("age", "integer")]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = JsonProvider::new("./data").await?;
    let users: Repository<User, _> = Repository::new(provider);

    let user = users.save(User {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
        deleted_at: None,
        created_at: None,
        updated_at: None,
    }).await?;

    let results = users.query()
        .where_contains("name", "Alice")
        .find()
        .await?;

    Ok(())
}
```

### Example: Using OrmEntity with Relations

```rust
use nosql_orm_derive::OrmEntity;

#[derive(OrmEntity)]
#[table_name("users")]
#[soft_delete]
#[timestamp]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub age: u32,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

---

## Manual vs Macro

### Manual Implementation

```rust
impl Entity for User {
    fn meta() -> EntityMeta {
        EntityMeta::new("users")
    }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for User {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::one_to_many("posts", "posts", "author_id"),
        ]
    }
}
```

### Macro Implementation

```rust
#[derive(Model)]
#[table_name("users")]
#[one_to_many("posts", "posts", "author_id")]
pub struct User { ... }
```

---

## Next Steps

- [07b-validators.md](07b-validators.md) - Validators
- [07c-soft-delete.md](07c-soft-delete.md) - Soft deletes
- [07d-timestamps.md](07d-timestamps.md) - Timestamps