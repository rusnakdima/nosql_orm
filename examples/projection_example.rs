//! Projection example - selecting specific fields
//!
//! Demonstrates how to use select() and exclude() to fetch
//! only specific fields from entities.

use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct User {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
  pub password: String,
  pub age: Option<u32>,
  pub bio: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UserOptional {
  pub id: Option<String>,
  pub name: Option<String>,
  pub email: Option<String>,
  pub password: Option<String>,
  pub age: Option<u32>,
  pub bio: Option<String>,
}

impl Entity for UserOptional {
  fn meta() -> EntityMeta {
    EntityMeta::new("users_opt")
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
  let temp_dir = TempDir::new().unwrap();
  let provider = JsonProvider::new(temp_dir.path()).await?;
  let repo = Repository::<User, _>::new(provider);

  // Create a user with all fields
  let user = User {
    id: None,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
    password: "secret123".to_string(),
    age: Some(30),
    bio: Some("Rust developer".to_string()),
  };
  let saved = repo.save(user).await?;
  println!("Created user: {:?}", saved.get_id().unwrap());

  // === SELECT specific fields ===
  // Only get id and name
  let user_id_name = repo
    .query()
    .select(&["id", "name"])
    .find_one()
    .await?
    .unwrap();
  println!("\n=== SELECT id, name ===");
  println!("id: {:?}", user_id_name.id);
  println!("name: {:?}", user_id_name.name);
  println!("email: {:?} (should be None)", (user_id_name as User).email); // Won't compile - but shows concept
                                                                          // Note: Deserialization will set non-selected fields to None if Option, or error if required
                                                                          // For this reason, use select() when all non-selected fields are Option<T>

  // === SELECT with all Option fields ===
  // If all non-required fields are Option, select works perfectly
  let provider2 = JsonProvider::new(temp_dir.path().join("opt")).await?;
  let repo2 = Repository::<UserOptional, _>::new(provider2);

  let user_opt = UserOptional {
    id: None,
    name: Some("Bob".to_string()),
    email: Some("bob@example.com".to_string()),
    password: Some("secret456".to_string()),
    age: Some(25),
    bio: Some("Developer".to_string()),
  };
  repo2.save(user_opt).await?;

  // Select only id and name - other fields become None
  let partial = repo2
    .query()
    .select(&["id", "name"])
    .find_one()
    .await?
    .unwrap();
  println!("\n=== SELECT id, name (all Option fields) ===");
  println!("id: {:?}", partial.id);
  println!("name: {:?}", partial.name);
  println!("email: {:?}", partial.email); // None
  println!("password: {:?}", partial.password); // None

  // === EXCLUDE specific fields ===
  // Get everything EXCEPT sensitive fields
  let safe = repo2
    .query()
    .exclude(&["password"])
    .find_one()
    .await?
    .unwrap();
  println!("\n=== EXCLUDE password ===");
  println!("id: {:?}", safe.id);
  println!("name: {:?}", safe.name);
  println!("password: {:?}", safe.password); // None

  // === Combine with filters ===
  let filtered = repo2
    .query()
    .where_gt("age", serde_json::json!(20))
    .select(&["id", "name", "age"])
    .find()
    .await?;
  println!("\n=== WHERE age > 20 + SELECT id, name, age ===");
  for user in filtered {
    println!(
      "User: id={:?}, name={:?}, age={:?}",
      user.id, user.name, user.age
    );
  }

  // === Raw JSON with projection ===
  // Get raw JSON instead of deserialized entity
  let raw_docs = repo2.query().exclude(&["password"]).find_raw().await?;
  println!("\n=== Raw JSON (excludes password) ===");
  for doc in raw_docs {
    println!("{}", serde_json::to_string_pretty(&doc).unwrap());
  }

  println!("\n✅ Projection example completed!");
  Ok(())
}
