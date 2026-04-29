//! Demonstrates full CRUD + relations with the JSON provider.
//!
//! Run: `cargo run --example json_example`

use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

// ── Entities ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct User {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
  pub age: u32,
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

impl WithRelations for User {
  fn relations() -> Vec<RelationDef> {
    vec![]
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Post {
  pub id: Option<String>,
  pub title: String,
  pub body: String,
  pub author_id: String,    // FK → User.id
  pub tag_ids: Vec<String>, // FK[] → Tag.id  (many-to-many)
  pub deleted_at: Option<DateTime<Utc>>,
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

impl SoftDeletable for Post {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    self.deleted_at = deleted_at;
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Tag {
  pub id: Option<String>,
  pub name: String,
}

impl WithRelations for Tag {
  fn relations() -> Vec<RelationDef> {
    vec![]
  }
}

impl Entity for Tag {
  fn meta() -> EntityMeta {
    EntityMeta::new("tags")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> OrmResult<()> {
  let tmp = tempfile::tempdir().unwrap();
  let provider = JsonProvider::new(tmp.path()).await?;

  // ── Repositories ─────────────────────────────────────────────────────────
  let users: Repository<User, _> = Repository::new(provider.clone());
  let tags: Repository<Tag, _> = Repository::new(provider.clone());
  let posts: RelationRepository<Post, _> = RelationRepository::new(provider.clone());

  // ── Seed data ─────────────────────────────────────────────────────────────
  let alice = users
    .save(User {
      id: None,
      name: "Alice".into(),
      email: "alice@example.com".into(),
      age: 30,
    })
    .await?;
  let bob = users
    .save(User {
      id: None,
      name: "Bob".into(),
      email: "bob@example.com".into(),
      age: 25,
    })
    .await?;

  let rust_tag = tags
    .save(Tag {
      id: None,
      name: "Rust".into(),
    })
    .await?;
  let orm_tag = tags
    .save(Tag {
      id: None,
      name: "ORM".into(),
    })
    .await?;

  let post1: Post = posts
    .repo()
    .save(Post {
      id: None,
      title: "Hello Rust".into(),
      body: "Rust is amazing!".into(),
      author_id: alice.id.clone().unwrap(),
      tag_ids: vec![rust_tag.id.clone().unwrap()],
      deleted_at: None,
    })
    .await?;

  let _post2: Post = posts
    .repo()
    .save(Post {
      id: None,
      title: "Building an ORM".into(),
      body: "Let's build a TypeORM clone in Rust.".into(),
      author_id: bob.id.clone().unwrap(),
      tag_ids: vec![rust_tag.id.clone().unwrap(), orm_tag.id.clone().unwrap()],
      deleted_at: None,
    })
    .await?;

  // ── Query: all users older than 26 ────────────────────────────────────────
  println!("\n=== Users older than 26 ===");
  let adults = users
    .query()
    .where_gt("age", serde_json::json!(26))
    .order_by(OrderBy::asc("name"))
    .find()
    .await?;
  for u in &adults {
    println!("  {:?}", u);
  }

  // ── Query: posts containing "Rust" in title ───────────────────────────────
  println!("\n=== Posts with 'Rust' in title ===");
  let rust_posts = posts
    .repo()
    .query()
    .where_contains("title", "Rust")
    .find()
    .await?;
  for p in &rust_posts {
    println!("  {:?}", p);
  }

  // ── Eager relation loading ────────────────────────────────────────────────
  println!("\n=== Post with author + tags (relations) ===");
  let loaded = posts
    .find_with_relations(post1.id.as_ref().unwrap(), &["author", "tags"])
    .await?
    .unwrap();

  println!("Post: {}", loaded.entity.title);
  if let Some(author) = loaded.one("author")? {
    println!("  Author: {}", author["name"]);
  }
  println!("  Tags:");
  for tag in loaded.many("tags")? {
    println!("    - {}", tag["name"]);
  }

  // ── Patch (partial update) ────────────────────────────────────────────────
  println!("\n=== Patch Alice's age ===");
  let patched = users
    .patch(alice.id.as_ref().unwrap(), serde_json::json!({ "age": 31 }))
    .await?;
  println!("  Alice's new age: {}", patched.age);

  // ── Delete ────────────────────────────────────────────────────────────────
  println!("\n=== Delete Bob ===");
  let removed = users.delete(bob.id.as_ref().unwrap()).await?;
  println!("  Removed: {}", removed);
  println!("  User count: {}", users.count().await?);

  // ── find_all_with_relations ───────────────────────────────────────────────
  println!("\n=== All posts with relations ===");
  let all = posts.find_all_with_relations(&["author", "tags"]).await?;
  for item in &all {
    let author_name = item
      .one("author")?
      .map(|a| a["name"].as_str().unwrap_or("?").to_string())
      .unwrap_or_else(|| "unknown".to_string());
    let tag_names: Vec<&str> = item
      .many("tags")?
      .iter()
      .filter_map(|t| t["name"].as_str())
      .collect();
    println!(
      "  '{}' by {} [tags: {}]",
      item.entity.title,
      author_name,
      tag_names.join(", ")
    );
  }

  println!("\n✓ All operations completed successfully.");
  Ok(())
}
