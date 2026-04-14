//! Demonstrates filtering by relation paths (e.g., `user.profile.name`)
//! and nested relation loading.
//!
//! Run: `cargo run --example relation_filter_example`

use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

// ── Entity: Profile ──────────────────────────────────────────────────────────

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

// ── Entity: User ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
  pub profile_id: String,
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

// ── Entity: Order ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
  pub id: Option<String>,
  pub order_number: String,
  pub total: f64,
  pub user_id: String,
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
    vec![
      // Order -> User (ManyToOne) with nested User -> Profile
      RelationDef::many_to_one("user", "users", "user_id").with_nested_relation(
        RelationDef::many_to_one("profile", "profiles", "profile_id"),
      ),
    ]
  }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
  // Use JSON file provider for this example
  let temp_dir = tempfile::tempdir().unwrap();
  let provider = JsonProvider::new(temp_dir.path().to_str().unwrap()).await?;

  // Create repositories
  let profiles: Repository<Profile, _> = Repository::new(provider.clone());
  let users: RelationRepository<User, _> = RelationRepository::new(provider.clone());
  let orders: RelationRepository<Order, _> = RelationRepository::new(provider.clone());

  println!("=== Setting up test data ===\n");

  // Create profiles
  let profile1 = profiles
    .save(Profile {
      id: None,
      bio: "Software engineer".to_string(),
      avatar_url: "https://example.com/avatar1.png".to_string(),
    })
    .await?;

  let profile2 = profiles
    .save(Profile {
      id: None,
      bio: "Product manager".to_string(),
      avatar_url: "https://example.com/avatar2.png".to_string(),
    })
    .await?;

  let _profile3 = profiles
    .save(Profile {
      id: None,
      bio: "Designer".to_string(),
      avatar_url: "https://example.com/avatar3.png".to_string(),
    })
    .await?;

  // Create users with profiles
  let user1 = users
    .save(User {
      id: None,
      name: "Alice Johnson".to_string(),
      email: "alice@example.com".to_string(),
      profile_id: profile1.get_id().unwrap(),
    })
    .await?;

  let user2 = users
    .save(User {
      id: None,
      name: "Bob Smith".to_string(),
      email: "bob@example.com".to_string(),
      profile_id: profile2.get_id().unwrap(),
    })
    .await?;

  let _user3 = users
    .save(User {
      id: None,
      name: "Charlie Brown".to_string(),
      email: "charlie@example.com".to_string(),
      profile_id: _profile3.get_id().unwrap(),
    })
    .await?;

  // Create orders
  let order1 = orders
    .save(Order {
      id: None,
      order_number: "ORD-001".to_string(),
      total: 99.99,
      user_id: user1.get_id().unwrap(),
    })
    .await?;

  let _order2 = orders
    .save(Order {
      id: None,
      order_number: "ORD-002".to_string(),
      total: 149.99,
      user_id: user2.get_id().unwrap(),
    })
    .await?;

  let _order3 = orders
    .save(Order {
      id: None,
      order_number: "ORD-003".to_string(),
      total: 29.99,
      user_id: user1.get_id().unwrap(), // Alice has another order
    })
    .await?;

  println!("Created 3 profiles, 3 users, and 3 orders.\n");

  // ── Feature 1: Filter by relation path ─────────────────────────────────────
  println!("=== Feature 1: Filter by relation path ===\n");

  println!("Finding orders where user's name is 'Alice Johnson'...\n");

  // Filter orders by user.name using relation path
  let orders_by_alice = orders
    .query_and_filter_with_relations(
      QueryBuilder::new().where_eq("user.name", serde_json::json!("Alice Johnson")),
      &["user"],
    )
    .await?;

  println!("Found {} orders by Alice:", orders_by_alice.len());
  for order in &orders_by_alice {
    println!(
      "  - {} (total: ${:.2})",
      order.entity.order_number, order.entity.total
    );
  }
  println!();

  // ── Feature 2: Nested relation loading ─────────────────────────────────────
  println!("=== Feature 2: Nested relation loading ===\n");

  // Load order with user relation (which includes nested profile due to RelationDef)
  let order_with_nested = orders
    .find_with_relations(&order1.get_id().unwrap(), &["user"])
    .await?
    .unwrap();

  println!("Order: {}", order_with_nested.entity.order_number);

  // Access user relation
  if let Some(user_val) = order_with_nested.one("user")? {
    println!("  User: {}", user_val["name"]);

    // Access nested profile via _nested field
    if let Some(nested) = user_val.get("_nested") {
      if let Some(profile_val) = nested.get("profile") {
        if !profile_val.is_null() {
          println!("  Profile Bio: {}", profile_val["bio"]);
          println!("  Avatar URL: {}", profile_val["avatar_url"]);
        }
      }
    }
  }
  println!();

  // ── Feature 3: Combined filtering and nested loading ───────────────────────
  println!("=== Feature 3: Combined filtering and nested loading ===\n");

  println!("Finding orders > $50 with user profile info...\n");

  let expensive_orders = orders
    .query_and_filter_with_relations(
      QueryBuilder::new().where_gt("total", serde_json::json!(50.0)),
      &["user"],
    )
    .await?;

  for order in &expensive_orders {
    println!(
      "Order: {} - ${:.2}",
      order.entity.order_number, order.entity.total
    );
    if let Some(user_val) = order.one("user")? {
      println!("  Customer: {}", user_val["name"]);
      // Access nested profile
      if let Some(nested) = user_val.get("_nested") {
        if let Some(profile_val) = nested.get("profile") {
          if !profile_val.is_null() {
            println!("  Bio: {}", profile_val["bio"]);
          }
        }
      }
    }
    println!();
  }

  // ── Feature 4: Filter by nested field with contains ────────────────────────
  println!("=== Feature 4: Filter by nested field with contains ===\n");

  println!("Finding users whose profile bio contains 'engineer'...\n");

  let engineers = users
    .query_and_filter_with_relations(
      QueryBuilder::new().where_contains("profile.bio", "engineer"),
      &["profile"],
    )
    .await?;

  for user in &engineers {
    println!("  - {}", user.entity.name);
    if let Some(profile_val) = user.one("profile")? {
      println!("    Bio: {}", profile_val["bio"]);
    }
  }
  println!();

  // ── Feature 5: Direct relation access (without filtering) ──────────────────
  println!("=== Feature 5: Direct relation access ===\n");

  println!("Loading all users with their profiles...\n");

  let all_users = users
    .query_and_filter_with_relations(QueryBuilder::new(), &["profile"])
    .await?;

  for user in &all_users {
    println!("User: {}", user.entity.name);
    if let Some(profile_val) = user.one("profile")? {
      println!("  Email: {}", user.entity.email);
      println!("  Bio: {}", profile_val["bio"]);
    }
    println!();
  }

  println!("✓ All relation features demonstrated successfully!");

  Ok(())
}
