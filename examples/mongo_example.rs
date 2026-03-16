//! Demonstrates full CRUD + relations with the MongoDB provider.
//!
//! Run: `cargo run --example mongo_example --features mongo`
//! Requires a MongoDB instance at mongodb://localhost:27017

use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
  pub id: Option<String>,
  pub name: String,
  pub price: f64,
  pub category_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[tokio::main]
async fn main() -> OrmResult<()> {
  let provider = MongoProvider::connect("mongodb://localhost:27017", "nosql_orm_demo").await?;

  let categories: Repository<Category, _> = Repository::new(provider.clone());
  let products: RelationRepository<Product, _> = RelationRepository::new(provider.clone());

  // Seed
  let electronics = categories
    .save(Category {
      id: None,
      name: "Electronics".into(),
    })
    .await?;

  let laptop = products
    .save(Product {
      id: None,
      name: "Laptop Pro".into(),
      price: 1299.99,
      category_id: electronics.id.clone().unwrap(),
    })
    .await?;

  // Query: products under $1500
  let affordable = products
    .repo()
    .query()
    .where_lt("price", serde_json::json!(1500.0))
    .find()
    .await?;
  println!("Affordable products: {:?}", affordable);

  // Relation loading
  let loaded = products
    .find_with_relations(laptop.id.as_ref().unwrap(), &["category"])
    .await?
    .unwrap();
  println!("Product: {}", loaded.entity.name);
  if let Some(cat) = loaded.one("category")? {
    println!("Category: {}", cat["name"]);
  }

  // Cleanup
  products.delete(laptop.id.as_ref().unwrap()).await?;
  categories.delete(electronics.id.as_ref().unwrap()).await?;

  println!("\n✓ MongoDB operations completed.");
  Ok(())
}
