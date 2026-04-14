# nosql_orm

A **TypeORM-inspired ORM for Rust** that works with both **JSON files** and **MongoDB** behind a single, unified API. Inspired by TypeORM's design philosophy: repositories, typed entities, fluent queries, and eager relation loading — all in idiomatic async Rust.

---

## Features

| Feature | JSON | MongoDB |
|---------|------|---------|
| Full CRUD | ✅ | ✅ |
| Fluent query builder | ✅ | ✅ |
| Sorting & pagination | ✅ | ✅ |
| Dot-notation field access | ✅ | — |
| OneToOne / ManyToOne | ✅ | ✅ |
| OneToMany | ✅ | ✅ |
| ManyToMany (embedded ids) | ✅ | ✅ |
| Partial update (`patch`) | ✅ | ✅ |
| Auto-generated UUIDs | ✅ | ✅ |

---

## Installation

```toml
[dependencies]
# JSON only (default):
nosql_orm = { version = "0.1", git = "https://github.com/rusnakdima/nosql_orm" }

# MongoDB only:
nosql_orm = { version = "0.1", git = "...", default-features = false, features = ["mongo"] }

# Both:
nosql_orm = { version = "0.1", git = "...", features = ["full"] }
```

---

## Quick Start

### 1. Define your entity

```rust
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
    pub age: u32,
}

// Implement Entity — or use the #[derive(Entity)] proc-macro (see below)
impl Entity for User {
    fn meta() -> EntityMeta { EntityMeta::new("users") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}
```

### 2. Connect and create a repository

```rust
use nosql_orm::prelude::*;

// JSON provider — reads/writes from ./data/*.json
let provider = JsonProvider::new("./data").await?;

// MongoDB provider
// let provider = MongoProvider::connect("mongodb://localhost:27017", "mydb").await?;

let users: Repository<User, _> = Repository::new(provider.clone());
```

### 3. CRUD

```rust
// INSERT (id auto-generated)
let user = users.save(User { id: None, name: "Alice".into(), email: "a@b.com".into(), age: 30 }).await?;

// GET by id
let found = users.get_by_id(user.id.as_ref().unwrap()).await?;

// UPDATE
let updated = users.save(User { id: user.id.clone(), age: 31, ..user.clone() }).await?;

// PATCH (partial update)
let patched = users.patch(user.id.as_ref().unwrap(), serde_json::json!({ "age": 32 })).await?;

// DELETE
users.delete(user.id.as_ref().unwrap()).await?;

// COUNT
let n = users.count().await?;
```

### 4. Fluent query builder

```rust
let results = users.query()
    .where_gt("age", serde_json::json!(18))
    .where_contains("email", "@example.com")
    .order_by(OrderBy::asc("name"))
    .skip(0)
    .limit(10)
    .find()
    .await?;
```

Available filters: `where_eq`, `where_ne`, `where_gt`, `where_lt`, `where_contains`,
`where_starts_with`, `where_in`, and raw `filter(Filter::And(...))`.

### 5. Relations

Declare relations on your entity:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub author_id: String,      // FK → users.id
    pub tag_ids: Vec<String>,   // FK[] → tags.id
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
```

Then use `RelationRepository`:

```rust
let posts: RelationRepository<Post, _> = RelationRepository::new(provider.clone());

// Load one post with author + tags
let item = posts
    .find_with_relations(&post_id, &["author", "tags"])
    .await?
    .unwrap();

println!("Post: {}", item.entity.title);

// Single (ManyToOne / OneToOne)
if let Some(author) = item.one("author")? {
    println!("Author: {}", author["name"]);
}

// Many (OneToMany / ManyToMany)
for tag in item.many("tags")? {
    println!("Tag: {}", tag["name"]);
}

// Load all posts with relations
let all = posts.find_all_with_relations(&["author", "tags"]).await?;

// Query with relations
let rust_posts = posts
    .query_with_relations(
        QueryBuilder::new().where_contains("title", "Rust"),
        &["author"],
    )
    .await?;
```

#### Relation types

| Method | When to use |
|--------|------------|
| `RelationDef::many_to_one("name", "collection", "local_fk")` | Post → User (post.author_id) |
| `RelationDef::one_to_many("name", "collection", "foreign_fk")` | User → Posts (post.user_id) |
| `RelationDef::one_to_one("name", "collection", "local_fk")` | User → Profile (user.profile_id) |
| `RelationDef::many_to_many("name", "collection", "ids_field")` | Post ↔ Tags (post.tag_ids[]) |

---

## Provider Config API

```rust
use nosql_orm::provider::ProviderConfig;

// JSON
let provider = JsonProvider::new("/path/to/data").await?;

// MongoDB via config struct
let config = ProviderConfig::new("mongodb://localhost:27017")
    .with_database("myapp");
let provider = MongoProvider::from_config(&config).await?;
```

---

## Swap providers without changing business logic

```rust
async fn run<P: DatabaseProvider>(provider: P) -> OrmResult<()> {
    let users: Repository<User, P> = Repository::new(provider);
    let user = users.save(User { id: None, name: "Test".into(), email: "t@t.com".into(), age: 20 }).await?;
    println!("{:?}", user);
    Ok(())
}

// Works with both:
run(JsonProvider::new("./data").await?).await?;
run(MongoProvider::connect("mongodb://localhost:27017", "mydb").await?).await?;
```

---

## Running examples

```bash
# JSON example (no external services needed)
cargo run --example json_example

# MongoDB example
cargo run --example mongo_example --features mongo
```

---

## Project layout

```
src/
├── lib.rs            — public API / re-exports
├── entity.rs         — Entity trait + EntityMeta
├── provider.rs       — DatabaseProvider trait
├── repository.rs     — Repository<E, P> + RelationRepository<E, P>
├── query.rs          — QueryBuilder + Filter
├── relations.rs      — RelationDef, WithRelations, RelationLoader, WithLoaded
├── utils.rs          — UUID generation
└── providers/
    ├── json.rs       — JSON file provider (feature = "json")
    └── mongo.rs      — MongoDB provider  (feature = "mongo")
```
