# Examples

Complete working examples demonstrating nosql_orm features.

---

## Table of Contents

1. [Basic Examples](#basic-examples)
2. [CRUD Examples](#crud-examples)
3. [Query Examples](#query-examples)
4. [Relation Examples](#relation-examples)
5. [Feature Examples](#feature-examples)
6. [Provider Examples](#provider-examples)

---

## Basic Examples

### Basic Entity

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

### Entity with Macro

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("users")]
pub struct User {
    pub id: Option<String>,
    pub name: String,
    pub email: String,
}
```

---

## CRUD Examples

### Full CRUD

```rust
#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = JsonProvider::new("./data").await?;
    let repo: Repository<User, _> = Repository::new(provider);

    // CREATE
    let user = repo.save(User {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).await?;

    // READ
    let found = repo.find_by_id(user.id.as_ref().unwrap()).await?;

    // UPDATE
    let updated = repo.save(User {
        id: user.id.clone(),
        name: "Alice Updated".into(),
        ..user
    }).await?;

    // PATCH
    let patched = repo.patch(
        user.id.as_ref().unwrap(),
        serde_json::json!({ "email": "new@example.com" })
    ).await?;

    // DELETE
    repo.delete(user.id.as_ref().unwrap()).await?;

    Ok(())
}
```

---

## Query Examples

### Simple Query

```rust
let results = repo.query()
    .where_eq("name", "Alice")
    .find()
    .await?;
```

### Complex Query

```rust
let results = repo.query()
    .where_in("status", vec!["active", "pending"])
    .where_gt("age", 18)
    .where_contains("email", "@example.com")
    .order_by(OrderBy::desc("created_at"))
    .limit(20)
    .find()
    .await?;
```

### Pagination

```rust
let page1 = repo.query()
    .order_by(OrderBy::asc("id"))
    .limit(20)
    .find_with_cursor(None)
    .await?;

let page2 = repo.query()
    .order_by(OrderBy::asc("id"))
    .limit(20)
    .find_with_cursor(page1.next_cursor)
    .await?;
```

---

## Relation Examples

### Entity with Relations

```rust
use nosql_orm::relations::{RelationDef, WithRelations};

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
```

### Loading Relations

```rust
let posts: RelationRepository<Post, _> = RelationRepository::new(provider);

let post = posts
    .find_with_relations(&post_id, &["author", "tags"])
    .await?
    .unwrap();

if let Some(author) = post.one("author")? {
    println!("Author: {}", author["name"]);
}

for tag in post.many("tags")? {
    println!("Tag: {}", tag["name"]);
}
```

---

## Feature Examples

### Soft Delete

```rust
repo.soft_delete("user-id").await?;
let users = repo.find_all().await?;           // Excludes deleted
let users = repo.find_all_including_deleted().await?;  // Includes deleted
repo.restore("user-id").await?;
```

### Migrations

```rust
let runner = MigrationRunner::new(provider);
runner.add_migration(SqlMigration::new(1, "create_users", "CREATE TABLE...", "DROP TABLE..."));
let applied = runner.run_all_pending().await?;
```

---

## Provider Examples

### JSON

```rust
let provider = JsonProvider::new("./data").await?;
```

### MongoDB

```rust
let provider = MongoProvider::connect("mongodb://localhost:27017", "mydb").await?;
```

### PostgreSQL

```rust
#[cfg(feature = "sql-postgres")]
let provider = PostgresProvider::connect("postgres://user:pass@localhost/db").await?;
```

### SQLite

```rust
#[cfg(feature = "sql-sqlite")]
let provider = SqliteProvider::connect("app.db").await?;
```

### MySQL

```rust
#[cfg(feature = "sql-mysql")]
let provider = MySqlProvider::connect("mysql://user:pass@localhost/db").await?;
```

---

## Complete Example

See `examples/json_example.rs` for a full working example with entities, CRUD, queries, and relations.

---

## Running Examples

```bash
# JSON example
cargo run --example json_example

# MongoDB example
cargo run --example mongo_example --features mongo

# SQL example
cargo run --example sql_example --features sql-sqlite
```