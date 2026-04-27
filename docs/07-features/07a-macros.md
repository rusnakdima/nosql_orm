# Macros

Derive macros for automatic Entity and Validate trait implementation.

---

## Table of Contents

1. [Model Macro](#model-macro)
2. [Validate Macro](#validate-macro)
3. [Index Attributes](#index-attributes)
4. [SQL Column Attributes](#sql-column-attributes)
5. [Complete Examples](#complete-examples)

---

## Model Macro

The `#[derive(Model)]` macro automatically implements the `Entity` trait and optionally `WithRelations` and `SoftDeletable`.

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
#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("users")]
#[soft_delete]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    // Auto-added by #[soft_delete]:
    pub deleted_at: Option<DateTime<Utc>>,
}

// Requires manual SoftDeletable impl (or use timestamp)
impl SoftDeletable for User {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) { self.deleted_at = d; }
}
```

### Example: With Timestamps

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("users")]
#[timestamp]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    // Auto-added by #[timestamp]:
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

### Example: With Custom ID

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Model)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
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
        email: "invalid".to_string(),
        name: "A".to_string(),
        age: 15,
    };
    
    // Validate returns Result
    if let Err(e) = user.validate() {
        println!("Validation failed: {}", e);
    }
}
```

---

## Complete Examples

### Example: Entity with Relations (Macro)

```rust
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("posts")]
#[many_to_one("author", "users", "author_id")]
#[many_to_many("categories", "categories", "category_ids")]
#[soft_delete]
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub author_id: String,
    pub category_ids: Vec<String>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl SoftDeletable for Post {
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.deleted_at
    }
    fn set_deleted_at(&mut self, d: Option<chrono::DateTime<chrono::Utc>>) {
        self.deleted_at = d;
    }
}
```

### Example: Full Stack with Macro

```rust
use nosql_orm_derive::{Model, Validate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
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
    // Auto-added by #[soft_delete]:
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    // Auto-added by #[timestamp]:
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl SoftDeletable for User {
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, d: Option<chrono::DateTime<chrono::Utc>>) { self.deleted_at = d; }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = JsonProvider::new("./data").await?;
    let users: Repository<User, _> = Repository::new(provider);

    // Create user
    let user = users.save(User {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
        deleted_at: None,
        created_at: None,
        updated_at: None,
    }).await?;

    // Query
    let results = users.query()
        .where_contains("name", "Alice")
        .find()
        .await?;

    Ok(())
}
```

### Example: Using Decorator Example

See `examples/decorator_example.rs`:

```rust
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("decorator_users")]
#[soft_delete]
#[timestamp]
pub struct DecoratorUser {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub age: u32,
}

// Note: Need to implement SoftDeletable manually with #[soft_delete]
impl SoftDeletable for DecoratorUser {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { None }
    fn set_deleted_at(&mut self, _: Option<DateTime<Utc>>) { }
}

// Note: Need to add timestamp fields manually with #[timestamp]
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