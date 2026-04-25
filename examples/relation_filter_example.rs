//! Demonstrates relation loading with filtering
//!
//! Run: `cargo run --example relation_filter_example`

use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
  pub id: Option<String>,
  pub bio: String,
  pub avatar_url: String,
}

impl Entity for Profile {
  fn meta() -> EntityMeta {
    EntityMeta::new("profiles")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

impl WithRelations for Profile {
  fn relations() -> Vec<RelationDef> {
    vec![]
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
  pub profile_id: String,
  pub deleted_at: Option<DateTime<Utc>>,
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
    vec![RelationDef::many_to_one(
      "profile",
      "profiles",
      "profile_id",
    )]
  }
}

impl SoftDeletable for User {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    self.deleted_at = deleted_at;
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
  pub id: Option<String>,
  pub order_number: String,
  pub total: f64,
  pub user_id: String,
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Order {
  fn meta() -> EntityMeta {
    EntityMeta::new("orders")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

impl WithRelations for Order {
  fn relations() -> Vec<RelationDef> {
    vec![RelationDef::many_to_one("user", "users", "user_id")]
  }
}

impl SoftDeletable for Order {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    self.deleted_at = deleted_at;
  }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
  let temp_dir = tempfile::tempdir().unwrap();
  let provider = JsonProvider::new(temp_dir.path().to_str().unwrap()).await?;

  let profiles: Repository<Profile, _> = Repository::new(provider.clone());
  let users: RelationRepository<User, _> = RelationRepository::new(provider.clone());
  let orders: RelationRepository<Order, _> = RelationRepository::new(provider.clone());

  println!("=== Setting up test data ===\n");

  let profile1: Profile = profiles
    .save(Profile {
      id: None,
      bio: "Software engineer".to_string(),
      avatar_url: "https://example.com/avatar1.png".to_string(),
    })
    .await?;

  let profile2: Profile = profiles
    .save(Profile {
      id: None,
      bio: "Product manager".to_string(),
      avatar_url: "https://example.com/avatar2.png".to_string(),
    })
    .await?;

  let _profile3: Profile = profiles
    .save(Profile {
      id: None,
      bio: "Designer".to_string(),
      avatar_url: "https://example.com/avatar3.png".to_string(),
    })
    .await?;

  let user1: User = users
    .repo()
    .save(User {
      id: None,
      name: "Alice Johnson".to_string(),
      email: "alice@example.com".to_string(),
      profile_id: profile1.get_id().unwrap(),
      deleted_at: None,
    })
    .await?;

  let user2: User = users
    .repo()
    .save(User {
      id: None,
      name: "Bob Smith".to_string(),
      email: "bob@example.com".to_string(),
      profile_id: profile2.get_id().unwrap(),
      deleted_at: None,
    })
    .await?;

  let _user3: User = users
    .repo()
    .save(User {
      id: None,
      name: "Charlie Brown".to_string(),
      email: "charlie@example.com".to_string(),
      profile_id: _profile3.get_id().unwrap(),
      deleted_at: None,
    })
    .await?;

  let order1: Order = orders
    .repo()
    .save(Order {
      id: None,
      order_number: "ORD-001".to_string(),
      total: 99.99,
      user_id: user1.get_id().unwrap(),
      deleted_at: None,
    })
    .await?;

  let _order2: Order = orders
    .repo()
    .save(Order {
      id: None,
      order_number: "ORD-002".to_string(),
      total: 149.99,
      user_id: user2.get_id().unwrap(),
      deleted_at: None,
    })
    .await?;

  let _order3: Order = orders
    .repo()
    .save(Order {
      id: None,
      order_number: "ORD-003".to_string(),
      total: 29.99,
      user_id: user1.get_id().unwrap(),
      deleted_at: None,
    })
    .await?;

  println!("Created 3 profiles, 3 users, and 3 orders.\n");

  println!("=== Feature 1: Find order with user relation ===\n");

  let order_with_user = orders
    .find_with_relations(order1.get_id().unwrap().as_str(), &["user"])
    .await?
    .unwrap();

  println!("Order: {}", order_with_user.entity.order_number);

  if let Some(user_val) = order_with_user.one("user")? {
    println!("  User: {}", user_val["name"]);
  }
  println!();

  println!("=== Feature 2: Find orders by user name ===\n");

  let orders_by_alice = orders
    .repo()
    .query()
    .where_eq("user_id", serde_json::json!(user1.get_id().unwrap()))
    .find()
    .await?;

  println!("Found {} orders by Alice:", orders_by_alice.len());
  for order in &orders_by_alice {
    println!("  - {} (total: ${:.2})", order.order_number, order.total);
  }
  println!();

  println!("=== Feature 3: Find all users with profile ===\n");

  let all_users = users.find_all_with_relations(&["profile"]).await?;

  for user in &all_users {
    println!("User: {}", user.entity.name);
    if let Some(profile_val) = user.one("profile")? {
      println!("  Bio: {}", profile_val["bio"]);
    }
    println!();
  }

  println!("=== Feature 4: Orders with expensive total > $50 ===\n");

  let expensive_orders: Vec<Order> = orders
    .repo()
    .query()
    .where_gt("total", serde_json::json!(50.0))
    .find()
    .await?;

  for order in &expensive_orders {
    println!("Order: {} - ${:.2}", order.order_number, order.total);
  }
  println!();

  println!("✓ All relation features demonstrated successfully!");

  Ok(())
}
