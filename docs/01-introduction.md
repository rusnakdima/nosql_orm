# nosql_orm — Introduction

`nosql_orm` is a TypeORM-inspired, async ORM for Rust with one unified,
type-safe API over heterogeneous storage backends: JSON files, MongoDB, Redis,
PostgreSQL, SQLite and MySQL.

Crate version: **0.9.0**. The crate name on crates.io is `nosql_orm`; the
library crate is `nosql_orm` and the derive crate is `nosql_orm_derive`
(re-exported from the main crate).

---

## Feature matrix

| Capability | JSON (`JsonProvider`) | JSON tree (`JsonTreeProvider`) | MongoDB (`MongoProvider`) | Redis (`RedisProvider`) | SQL (`Postgres`/`MySql`/`Sqlite`) |
|---|---|---|---|---|---|
| Full CRUD | ✅ | ✅ | ✅ | ✅ | ✅ |
| Query builder / `Filter` | ✅ (in-memory) | ✅ (in-memory) | ✅ (translated) | ✅ (in-memory) | ✅ (translated) |
| Relations (1:1, 1:N, N:M) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Batch insert | ✅ | ✅ | ✅ | ✅ | ✅ |
| Auto-generated UUIDs | ✅ | ✅ | ✅ | ✅ | ✅ |
| Field projection | ✅ | ✅ | ✅ | ✅ (in-memory) | ✅ |
| Indexes | no-op / empty | no-op / empty | ✅ | ❌ (`NotSupported`) | ✅ |
| Aggregation | ✅ (in-memory) | ❌ (`NotSupported`) | ✅ | ❌ | ❌ |
| Raw SQL (`execute_raw`) | ❌ | ❌ | ❌ | ❌ | ✅ |
| Transactions | ❌ | ❌ | ❌ | ❌ | SQLite ✅ (others via driver pools) |
| Schema sync (`sync_schema`) | ❌ (non-SQL) | ❌ | ❌ | ❌ | ✅ |

`Capability` (`transactions`, `indexes`, `aggregation`, `raw_queries`) is
queryable at runtime via `provider.capabilities()` / `provider.supports(...)`.
Operations outside a backend's contract return `OrmError::NotSupported`.

---

## Installation

```toml
[dependencies]
# JSON only (default)
nosql_orm = "0.9"

# MongoDB
nosql_orm = { version = "0.9", features = ["mongo"] }

# Redis
nosql_orm = { version = "0.9", features = ["redis"] }

# SQL (all three)
nosql_orm = { version = "0.9", features = ["sql"] }

# Everything
nosql_orm = { version = "0.9", features = ["full"] }
```

### Feature flags

| Flag | Effect |
|---|---|
| `default = ["json"]` | JSON provider is on by default |
| `json` | `JsonProvider`, `JsonTreeProvider` |
| `mongo` | `MongoProvider` (`dep:mongodb`, `dep:futures-util`, `dep:futures`) |
| `redis` | `RedisProvider` (`dep:redis`) |
| `sql` | all three SQL providers |
| `sql-postgres` | `PostgresProvider` |
| `sql-sqlite` | `SqliteProvider` |
| `sql-mysql` | `MySqlProvider` |
| `full` | `json`, `mongo`, `redis`, `sql` |
| `query_cache` | `cache::QueryCache` (off by default) |
| `embedded` | `embedded` module (off by default, opt-in) |
| `inheritance` | `inheritance` module (off by default, opt-in) |
| `lazy` | `lazy` module (off by default, opt-in) |

There are **no** `graphql`, `cdc`, `subscription` or `search` features: those
modules do not exist in this crate.

---

## Quick start

### 1. Define an entity

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
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}
```

### 2. Create a provider

```rust
let provider = JsonProvider::new("./data").await?;          // JSON
// let provider = MongoProvider::connect("mongodb://localhost:27017", "db").await?;
// let provider = RedisProvider::new("redis://localhost:6379").await?;
// let provider = SqliteProvider::connect("app.db").await?;
```

### 3. CRUD

```rust
let repo: Repository<User, _> = Repository::new(provider);

let user = repo.save(User { id: None, name: "Alice".into(), email: "a@x.com".into() }).await?;
let found = repo.find_by_id(user.id.as_ref().unwrap()).await?;
let patched = repo.patch(user.id.as_ref().unwrap(), serde_json::json!({ "name": "Alice 2" })).await?;
repo.delete(user.id.as_ref().unwrap()).await?;
let count = repo.count().await?;
```

`save` is an upsert: inserts when `id` is `None`, updates otherwise.

### 4. Query builder

```rust
let results = repo.query()
    .where_eq("email", "alice@example.com")
    .where_gt("age", 18)
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
    pub author_id: String,
    pub tag_ids: Vec<String>,
}

impl Entity for Post { /* ... */ }

impl WithRelations for Post {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::many_to_one("author", "users", "author_id"),
            RelationDef::many_to_many("tags", "tags", "tag_ids"),
        ]
    }
}

let posts: RelationRepository<Post, _> = RelationRepository::new(provider);
let post = posts.find_with_relations(&post_id, &["author", "tags"]).await?.unwrap();
if let Some(author) = post.one("author")? { println!("{}", author["name"]); }
```

`RelationRepository::find_with_relations` is only available when the entity
also implements `SoftDeletable` (see `src/repository/relations.rs`). For
non-soft-deletable entities use `repo()` plus manual relation loading.

---

## Using macros

```rust
use nosql_orm_derive::{Model, Validate};
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

`#[derive(Model)]` implements `Entity` (and `WithRelations` with an empty
list). `#[derive(Validate)]` implements the `Validate` trait from the
`#[validate(...)]` attributes. See
[07-features/07a-macros.md](07-features/07a-macros.md).

---

## Documentation map

```
docs/
├── 00-table-of-contents.md
├── 01-introduction.md
├── 02-entity.md
├── 03-provider.md
├── 04-repository.md
├── 05-query-builder.md
├── 06-relations.md
├── 07-features/          # per-feature deep dives
├── 08-examples.md
├── 09-api-reference.md
├── ARCHITECTURE.md
└── MODULE.md
```

---

## Next steps

- [02-entity.md](02-entity.md) — Entity trait and metadata
- [03-provider.md](03-provider.md) — Providers and feature flags
- [04-repository.md](04-repository.md) — Repository CRUD
- [05-query-builder.md](05-query-builder.md) — Querying
- [06-relations.md](06-relations.md) — Relations
- [07-features/](07-features/) — Advanced features
- [08-examples.md](08-examples.md) — Full examples
- [09-api-reference.md](09-api-reference.md) — API quick reference