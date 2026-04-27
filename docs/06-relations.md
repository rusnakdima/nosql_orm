# Relations

Eager loading and relationship management between entities.

---

## Table of Contents

1. [RelationDef](#relationdef)
2. [Relation Types](#relation-types)
3. [WithRelations](#withrelations)
4. [RelationLoader](#relationloader)
5. [WithLoaded](#withloaded)
6. [Registration](#registration)

---

## RelationDef

Defines a relationship between entities.

### Creating Relations

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
```

### Methods

```rust
let relation = RelationDef::many_to_one("author", "users", "author_id");
let relation = RelationDef::one_to_many("posts", "posts", "user_id");
let relation = RelationDef::one_to_one("profile", "profiles", "profile_id");
let relation = RelationDef::many_to_many("tags", "tags", "tag_ids");
```

---

## Relation Types

### OneToOne

One record relates to one record.

```
User ─────── Profile
  id    ──── user_id
```

```rust
RelationDef::one_to_one("profile", "profiles", "profile_id")
```

### ManyToOne (Foreign Key)

Many records relate to one record.

```
Post ─────── User
 author_id  ──── id
```

```rust
RelationDef::many_to_one("author", "users", "author_id")
```

### OneToMany

One record relates to many records.

```
User ─────── Posts
   id ────── user_id
```

```rust
RelationDef::one_to_many("posts", "posts", "author_id")
```

### ManyToMany

Many records relate to many records via join field.

```
Post ────── Tag
 tag_ids ── id
```

```rust
RelationDef::many_to_many("tags", "tags", "tag_ids")
```

---

## WithRelations

Trait for entities declaring their relations.

### Implementing

```rust
use nosql_orm::prelude::*;
use nosql_orm::relations::{RelationDef, WithRelations};
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
```

---

## RelationLoader

Batch loads relations efficiently.

### Loading Single Relation

```rust
let loader = RelationLoader::new(provider);

// Load one relation
let result = loader.load_relation(&doc, &relation_def).await?;
```

### Loading Multiple Relations (Batch)

```rust
// Load ManyToOne for multiple docs at once
let results = loader.load_many(docs, &relation, true).await?;
```

### Loading Nested

```rust
// Load nested relations (e.g., tasks.subtasks.comments)
let results = loader.load_nested(docs, &["tasks", "subtasks"], true).await?;
```

### Loading with Relations on Docs

```rust
let results = loader.load_relations_on_docs(
    docs,
    "posts",
    &["author", "tags"],
    true
).await?;
```

---

## WithLoaded

Entity with loaded relations.

### Structure

```rust
pub struct WithLoaded<E: Entity> {
    pub entity: E,
    pub loaded: HashMap<String, RelationValue>,
}
```

### Accessing Relations

```rust
let post = relation_repo
    .find_with_relations("post-id-123", &["author", "tags"])
    .await?
    .unwrap();

// Entity
println!("{}", post.entity.title);

// Get single relation (ManyToOne/OneToOne)
if let Some(author) = post.one("author")? {
    println!("Author: {}", author["name"]);
}

// Get many relation (OneToMany/ManyToMany)
for tag in post.many("tags")? {
    println!("Tag: {}", tag["name"]);
}
```

### Methods

```rust
impl<E: Entity> WithLoaded<E> {
    pub fn new(entity: E) -> Self;

    pub fn one(&self, name: &str) -> OrmResult<Option<&Value>>;
    pub fn many(&self, name: &str) -> OrmResult<&[Value]>;
    pub fn get(&self, path: &str) -> Option<&RelationValue>;
    pub fn keys(&self) -> Vec<&String>;
    pub fn has(&self, name: &str) -> bool;
}
```

---

## Registration

Relations must be registered before using RelationRepository.

### register_collection_relations()

```rust
use nosql_orm::prelude::*;
use nosql_orm::relations::register_collection_relations;

register_collection_relations("posts", vec![
    RelationDef::many_to_one("author", "users", "author_id"),
    RelationDef::many_to_many("tags", "tags", "tag_ids"),
]);
```

### register_relations_for_entity()

```rust
use nosql_orm::prelude::*;
use nosql_orm::relations::register_relations_for_entity;

register_relations_for_entity::<Post>();
```

---

## Examples

### Basic Relations

```rust
use chrono::{DateTime, Utc};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: Option<String>,
    pub name: String,
}

impl Entity for Tag {
    fn meta() -> EntityMeta { EntityMeta::new("tags") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let provider = JsonProvider::new("./data").await?;

    let users = Repository::<User, _>::new(provider.clone());
    let tags = Repository::<Tag, _>::new(provider.clone());
    let posts: RelationRepository<Post, _> = RelationRepository::new(provider.clone());

    // Register relations
    register_relations_for_entity::<Post>();

    // Create data
    let alice = users.save(User {
        id: None,
        name: "Alice".into(),
        email: "alice@example.com".into(),
    }).await?;

    let tag = tags.save(Tag {
        id: None,
        name: "Rust".into(),
    }).await?;

    let post = posts.repo().save(Post {
        id: None,
        title: "Hello Rust".into(),
        author_id: alice.id.unwrap(),
        tag_ids: vec![tag.id.unwrap()],
    }).await?;

    // Load with relations
    let loaded = posts
        .find_with_relations(&post.id.unwrap(), &["author", "tags"])
        .await?
        .unwrap();

    println!("Post: {}", loaded.entity.title);
    if let Some(author) = loaded.one("author")? {
        println!("Author: {}", author["name"]);
    }
    for tag in loaded.many("tags")? {
        println!("Tag: {}", tag["name"]);
    }

    Ok(())
}
```

### Nested Relations

```rust
// Load Todo with tasks
let todo = todos.find_with_relations(&todo_id, &["tasks"]).await?;

// Load Todo with nested relations (tasks and their subtasks)
let todo = todos.find_with_relations(&todo_id, &["tasks"]).await?;

// Custom nested loading via RelationLoader
let loader = RelationLoader::new(provider);
let docs = loader.load_nested(docs, &["tasks", "subtasks"], true).await?;
```

### Cascade Delete

```rust
let relation = RelationDef::many_to_one("author", "users", "author_id")
    .on_delete(SqlOnDelete::Cascade);

// Now deleting a user cascades to posts
let deleted = users.delete("user-id").await?;
```

---

## Next Steps

- [07-features/07c-soft-delete.md](07-features/07c-soft-delete.md) - Soft deletes
- [07-features/07h-cascade.md](07-features/07h-cascade.md) - Cascade delete
- [07-features/07i-lazy.md](07-features/07i-lazy.md) - Lazy loading