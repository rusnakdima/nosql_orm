# Repository Pattern

The Repository pattern provides CRUD operations between entities and providers.

---

## Table of Contents

1. [Repository](#repository)
2. [CRUD Operations](#crud-operations)
3. [Query Methods](#query-methods)
4. [RelationRepository](#relationrepository)
5. [Index Management](#index-management)
6. [SQL Schema](#sql-schema)

---

## Repository

### Creating a Repository

```rust
let repo: Repository<User, JsonProvider> = Repository::new(provider);

// Or with type inference
let repo = Repository::<User, _>::new(provider);
```

### Repository Structure

```rust
pub struct Repository<E, P>
where
    E: Entity,
    P: DatabaseProvider,
{
    pub(crate) provider: P,
    _phantom: PhantomData<E>,
}
```

---

## CRUD Operations

### Insert

Insert a new entity (id auto-generated if not set):

```rust
let user = repo.insert(User {
    id: None,
    name: "Alice".into(),
    email: "alice@example.com".into(),
}).await?;
```

### Save

Insert or update based on id presence:

```rust
// Insert (no id)
let user = repo.save(User {
    id: None,
    name: "Alice".into(),
    email: "alice@example.com".into(),
}).await?;

// Update (has id)
let user = repo.save(User {
    id: Some("existing-id".into()),
    name: "Updated".into(),
    email: "updated@example.com".into(),
}).await?;
```

### Find by ID

```rust
// Returns Option<T>
let user = repo.find_by_id("user-id-123").await?;

// Returns T or Error
let user = repo.get_by_id("user-id-123").await?;
```

### Find All

```rust
// Excludes soft-deleted (for SoftDeletable entities)
let users = repo.find_all().await?;

// Includes soft-deleted
let users = repo.find_all_including_deleted().await?;
```

### Update

Update an entity (requires id):

```rust
let updated = repo.update(User {
    id: Some("user-id".into()),
    name: "Updated Name".into(),
    email: "updated@example.com".into(),
}).await?;
```

### Patch

Partial update (merge fields):

```rust
let patched = repo.patch("user-id", serde_json::json!({
    "name": "Patched Name"
})).await?;
```

### Delete

Hard delete:

```rust
// By id
let deleted = repo.delete("user-id").await?;

// By entity
let deleted = repo.remove(&entity).await?;
```

### Soft Delete

For entities implementing `SoftDeletable`:

```rust
let deleted = repo.soft_delete("user-id").await?;
let restored = repo.restore("user-id").await?;
```

### Count

```rust
let count = repo.count().await?;
```

### Exists

```rust
let exists = repo.exists("user-id").await?;
```

---

## Query Methods

### query()

Start a fluent query:

```rust
let results = repo.query()
    .where_eq("name", "Alice")
    .where_gt("age", 18)
    .order_by(OrderBy::asc("name"))
    .limit(10)
    .find()
    .await?;
```

### query_including_deleted()

Query including soft-deleted:

```rust
let results = repo.query_including_deleted()
    .where_eq("name", "Alice")
    .find()
    .await?;
```

---

## RelationRepository

Extends Repository with relation loading.

### Creating RelationRepository

```rust
let posts: RelationRepository<Post, JsonProvider> = RelationRepository::new(provider);
```

### Find with Relations

```rust
// Load single entity with relations
let post = posts
    .find_with_relations("post-id", &["author", "tags"])
    .await?
    .unwrap();

// Access entity
println!("{}", post.entity.title);

// Access loaded single relation (ManyToOne/OneToOne)
if let Some(author) = post.one("author")? {
    println!("Author: {}", author["name"]);
}

// Access loaded collection relation (OneToMany/ManyToMany)
for tag in post.many("tags")? {
    println!("Tag: {}", tag["name"]);
}
```

### Find All with Relations

```rust
let posts = posts.find_all_with_relations(&["author", "tags"]).await?;

for post in posts {
    if let Some(author) = post.one("author")? {
        println!("Author: {}", author["name"]);
    }
}
```

### Query with Relations

```rust
let posts = posts.query_with_relations(
    QueryBuilder::new()
        .where_contains("title", "Rust")
        .limit(10),
    &["author", "tags"]
).await?;
```

---

## Index Management

### create_index()

```rust
repo.create_index(NosqlIndex::single("email", 1)).await?;
repo.create_index(NosqlIndex::unique("email"))?;
repo.create_index(NosqlIndex::compound(&[("field1", 1), ("field2", -1)]))?;
repo.create_index(NosqlIndex::text(&[("title", 10), ("body", 5)]))?;
repo.create_index(NosqlIndex::ttl("created_at", 3600))?;
```

### list_indexes()

```rust
let indexes = repo.list_indexes().await?;

for idx in indexes {
    println!("{}", idx.name);
}
```

### drop_index()

```rust
repo.drop_index("idx_email").await?;
```

### sync_indexes()

Create indexes from entity definition:

```rust
let created = repo.sync_indexes().await?;
```

---

## SQL Schema

### sync_schema()

Sync table schema from entity:

```rust
repo.sync_schema().await?;
```

### execute_sql()

Execute raw SQL:

```rust
repo.execute_sql("TRUNCATE users CASCADE").await?;
```

---

## Examples

### Full CRUD Example

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
    println!("Created: {:?}", user.id);

    // READ
    let found = repo.find_by_id(user.id.as_ref().unwrap()).await?;
    println!("Found: {:?}", found);

    // UPDATE
    let updated = repo.save(User {
        id: user.id.clone(),
        name: "Alice Updated".into(),
        email: "updated@example.com".into(),
    }).await?;
    println!("Updated: {:?}", updated.name);

    // PATCH
    let patched = repo.patch(
        user.id.as_ref().unwrap(),
        serde_json::json!({ "email": "patched@example.com" })
    ).await?;
    println!("Patched email: {:?}", patched.email);

    // COUNT & EXISTS
    println!("Count: {}", repo.count().await?);
    println!("Exists: {}", repo.exists(user.id.as_ref().unwrap()).await?);

    // QUERY
    let results = repo.query()
        .where_contains("name", "Alice")
        .order_by(OrderBy::asc("name"))
        .limit(10)
        .find()
        .await?;
    println!("Found {} users", results.len());

    // DELETE
    let deleted = repo.delete(user.id.as_ref().unwrap()).await?;
    println!("Deleted: {}", deleted);

    Ok(())
}
```

### With Relations

```rust
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

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
    let provider = JsonProvider::new("./data").await?;
    let posts: RelationRepository<Post, _> = RelationRepository::new(provider);

    // Register relations
    register_relations_for_entity::<Post>();

    // Load with relations
    let post = posts
        .find_with_relations("post-id", &["author", "tags"])
        .await?
        .unwrap();

    // Entity
    println!("Title: {}", post.entity.title);

    // ManyToOne/OneToOne
    if let Some(author) = post.one("author")? {
        println!("Author: {}", author["name"]);
    }

    // OneToMany/ManyToMany
    for tag in post.many("tags")? {
        println!("Tag: {}", tag["name"]);
    }

    Ok(())
}
```

---

## Next Steps

- [05-query-builder.md](05-query-builder.md) - Query builder
- [06-relations.md](06-relations.md) - Relations
- [07-features/](07-features/) - Feature documentation