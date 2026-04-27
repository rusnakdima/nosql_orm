# nosql_orm - Introduction

A TypeORM-inspired ORM for NoSQL and SQL databases with a unified, type-safe API in Rust. Supports JSON files, MongoDB, Redis, PostgreSQL, SQLite, and MySQL.

---

## Features

### Core Features
| Feature | JSON | MongoDB | Redis | SQL |
|--------|------|--------|------|------|
| Full CRUD | ✅ | ✅ | ✅ | ✅ |
| Fluent query builder | ✅ | ✅ | — | ✅ |
| Relations (1:1, 1:N, N:M) | ✅ | ✅ | ✅ | ✅ |
| Batch operations | ✅ | ✅ | ✅ | ✅ |
| Auto-generated UUIDs | ✅ | ✅ | ✅ | ✅ |
| Field projection | ✅ | ✅ | — | ✅ |
| Indexes | ✅ | ✅ | ✅ | ✅ |

### Advanced Features
| Feature | Status |
|---------|--------|
| Soft deletes | ✅ |
| Timestamps (auto created_at/updated_at) | ✅ |
| Validators | ✅ |
| Migrations | ✅ |
| Event listeners | ✅ |
| Cascade delete | ✅ |
| Lazy loading | ✅ |
| Embedded entities | ✅ |
| Table inheritance | ✅ |
| Full-text search | ✅ |
| Aggregation pipeline | ✅ |
| Change Data Capture (CDC) | ✅ |
| GraphQL integration | ✅ |
| Pub/Sub | ✅ |
| Query caching | ✅ |
| Query logging | ✅ |

---

## Installation

```toml
[dependencies]
# JSON only (default)
nosql_orm = "0.6"

# MongoDB
nosql_orm = { version = "0.6", features = ["mongo"] }

# Redis
nosql_orm = { version = "0.6", features = ["redis"] }

# All NoSQL providers
nosql_orm = { version = "0.6", features = ["full"] }

# SQL providers
nosql_orm = { version = "0.6", features = ["sql"] }

# Full features
nosql_orm = { version = "0.6", features = ["full", "sql"] }
```

### Feature Flags

```toml
[features]
default = ["json"]
json = []                    # JSON file provider (default)
mongo = ["dep:mongodb", "dep:futures-util"]  # MongoDB provider
redis = ["dep:redis"]       # Redis provider
full = ["json", "mongo", "redis"]

query_cache = []             # Query caching
validators = []              # Entity validation

# SQL Providers
sql-postgres = ["dep:tokio-postgres", "dep:deadpool-postgres", "dep:base64"]
sql-sqlite = ["dep:rusqlite", "dep:base64"]
sql-mysql = ["dep:mysql_async", "dep:base64"]
sql = ["sql-postgres", "sql-sqlite", "sql-mysql"]
```

---

## Quick Start

### 1. Define an Entity

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

### 2. Create a Provider

```rust
// JSON provider (embedded, zero-config)
let provider = JsonProvider::new("./data").await?;

// MongoDB provider
let provider = MongoProvider::connect("mongodb://localhost:27017", "mydb").await?;

// Redis provider
let provider = RedisProvider::connect("redis://localhost:6379").await?;

// SQL providers
let provider = PostgresProvider::connect("postgres://user:pass@localhost/db").await?;
let provider = SqliteProvider::connect("app.db").await?;
let provider = MySqlProvider::connect("mysql://user:pass@localhost/db").await?;
```

### 3. CRUD Operations

```rust
let repo: Repository<User, _> = Repository::new(provider);

// INSERT (id auto-generated)
let user = repo.save(User {
    id: None,
    name: "Alice".into(),
    email: "alice@example.com".into(),
}).await?;

// FIND by id
let found = repo.find_by_id(user.id.as_ref().unwrap()).await?;

// UPDATE
let updated = repo.save(User {
    id: user.id.clone(),
    name: "Alice Updated".into(),
    ..user
}).await?;

// PATCH (partial update)
let patched = repo.patch(user.id.as_ref().unwrap(), 
    serde_json::json!({ "name": "Alice Patched" })
).await?;

// DELETE
repo.delete(user.id.as_ref().unwrap()).await?;

// COUNT
let count = repo.count().await?;
```

### 4. Query Builder

```rust
let results = repo.query()
    .where_eq("email", "alice@example.com")
    .where_gt("age", 18)
    .where_contains("name", "Alice")
    .order_by(OrderBy::asc("name"))
    .skip(0)
    .limit(10)
    .find()
    .await?;
```

### 5. Relations

```rust
use nosql_orm::relations::{RelationDef, WithRelations};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub author_id: String,      // FK → users.id
    pub tag_ids: Vec<String>, // FK[] → tags.id (many-to-many)
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
```

```rust
let posts: RelationRepository<Post, _> = RelationRepository::new(provider);

let post = posts.find_with_relations(&post_id, &["author", "tags"]).await?.unwrap();

if let Some(author) = post.one("author")? {
    println!("Author: {}", author["name"]);
}

for tag in post.many("tags")? {
    println!("Tag: {}", tag["name"]);
}
```

---

## Using Macros

Instead of manually implementing traits, use the `#[derive(Model)]` and `#[derive(Validate)]` macros:

```rust
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("users")]
#[soft_delete]
#[timestamp]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

See [07-features/07a-macros.md](07-features/07a-macros.md) for detailed macro documentation.

---

## Why nosql_orm?

### Key Differentiators
1. **Unified API** - Single code works for NoSQL + SQL databases
2. **Zero-config storage** - Embedded JSON provider needs no setup
3. **Native async** - Built on Tokio for async Rust
4. **Type-safe** - Full Rust type safety
5. **Rich features** - Built-in GraphQL, Pub/Sub, CDC, migrations

### Comparison

| Feature | nosql_orm | TypeORM | Prisma | Django ORM |
|---------|-----------|--------|-------|--------|
| NoSQL support | ✅ MongoDB, Redis, JSON | ⚠️ Limited | ❌ | ❌ |
| Zero-config | ✅ JSON | ❌ | ⚠️ | ❌ |
| SQL support | ✅ PG, MySQL, SQLite | ✅ | ✅ | ✅ |
| Async first | ✅ | ⚠️ | ✅ | ❌ |
| Built-in GraphQL | ✅ | ❌ | ❌ | ❌ |
| Built-in Pub/Sub | ✅ | ❌ | ❌ | ❌ |
| CDC | ✅ | ❌ | ❌ | ❌ |
| Migrations | ✅ | ✅ | ✅ | ✅ |
| Relations | ✅ | ✅ | ✅ | ✅ |

---

## Documentation Structure

```
docs/
├── 00-table-of-contents.md    # This file
├── 01-introduction.md        # Introduction (this file)
├── 02-entity.md            # Entity trait
├── 03-provider.md         # Providers (JSON, MongoDB, Redis, SQL)
├── 04-repository.md        # Repository pattern
├── 05-query-builder.md   # Query builder
├── 06-relations.md        # Relations
├── 07-features/          # Feature modules
│   ├── README.md
│   ├── 07a-macros.md
│   ├── 07b-validators.md
│   ├── ...
├── 08-examples.md        # Examples
└── 09-api-reference.md  # API quick reference
```

---

## Next Steps

- Read [02-entity.md](02-entity.md) - Learn about Entity trait
- Read [03-provider.md](03-provider.md) - Understand providers
- Read [04-repository.md](04-repository.md) - Repository CRUD
- Read [05-query-builder.md](05-query-builder.md) - Query builder
- Read [06-relations.md](06-relations.md) - Relations
- Explore [07-features/](07-features/) - Advanced features
- Check [08-examples.md](08-examples.md) - Full examples