//! Demonstrates full CRUD + relations with the MongoDB provider.
//!
//! Run: `cargo run --example mongo_example --features mongo`
//! Requires a MongoDB instance at mongodb://localhost:27017

use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm::providers::MongoProvider;
use serde::{Deserialize, Serialize};

// ── Entities ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Product {
  pub id: Option<String>,
  pub name: String,
  pub price: f64,
  pub category_id: String, // FK → Category.id
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Product {
  fn meta() -> EntityMeta {
    EntityMeta::new("products")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

impl WithRelations for Product {
  fn relations() -> Vec<RelationDef> {
    vec![RelationDef::many_to_one(
      "category",
      "categories",
      "category_id",
    )]
  }
}

impl SoftDeletable for Product {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    self.deleted_at = deleted_at;
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Category {
  pub id: Option<String>,
  pub name: String,
}

impl Entity for Category {
  fn meta() -> EntityMeta {
    EntityMeta::new("categories")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

impl WithRelations for Category {
  fn relations() -> Vec<RelationDef> {
    vec![]
  }
}

// ── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> OrmResult<()> {
  let provider = MongoProvider::connect("mongodb://localhost:27017", "nosql_orm_demo").await?;

  // ── Repositories ─────────────────────────────────────────────────────────
  let categories: Repository<Category, _> = Repository::new(provider.clone());
  let products: RelationRepository<Product, _> = RelationRepository::new(provider.clone());

  // ── Seed data ────────────────────────────────────────────────────────────
  let electronics = categories
    .save(Category {
      id: None,
      name: "Electronics".into(),
    })
    .await?;

  let laptop = products
    .repo()
    .save(Product {
      id: None,
      name: "Laptop Pro".into(),
      price: 1299.99,
      category_id: electronics.id.clone().unwrap(),
      deleted_at: None,
    })
    .await?;

  // ── Query: products under $1500 ──────────────────────────────────────────
  let affordable = products
    .repo()
    .query()
    .where_lt("price", serde_json::json!(1500.0))
    .find()
    .await?;
  println!("Affordable products: {:?}", affordable);

  // ── Eager relation loading ────────────────────────────────────────────────
  let loaded = products
    .find_with_relations(laptop.id.as_ref().unwrap(), &["category"])
    .await?
    .unwrap();

  println!("Product: {}", loaded.entity.name);
  if let Some(category) = loaded.one("category")? {
    println!("Category: {}", category["name"]);
  }

  // ── Delete ────────────────────────────────────────────────────────────────
  products.repo().delete(laptop.id.as_ref().unwrap()).await?;
  categories.delete(electronics.id.as_ref().unwrap()).await?;

  println!("\n✓ MongoDB operations completed.");
  Ok(())
}
