use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
  pub age: u32,
  pub created_at: chrono::DateTime<chrono::Utc>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
  pub id: Option<String>,
  pub title: String,
  pub content: String,
  pub author_id: String,
  pub location: Option<GeoPoint>,
  pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoPoint {
  pub r#type: String,
  pub coordinates: Vec<f64>,
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

#[tokio::main]
async fn main() -> OrmResult<()> {
  println!("=== NoSQL Index Example ===\n");

  let provider = JsonProvider::new("./data").await?;

  let repo: Repository<User, _> = Repository::new(provider.clone());

  repo.create_index(NosqlIndex::single("name", 1)).await?;
  repo
    .create_index(NosqlIndex::compound(&[("age", 1), ("name", -1)]))
    .await?;

  let post_repo: Repository<Post, _> = Repository::new(provider.clone());
  post_repo
    .create_index(NosqlIndex::text(&[("title", 10), ("content", 5)]))
    .await?;
  post_repo
    .create_index(NosqlIndex::geospatial_2dsphere("location"))
    .await?;
  post_repo
    .create_index(NosqlIndex::hashed("author_id"))
    .await?;
  post_repo
    .create_index(NosqlIndex::ttl("created_at", 3600))
    .await?;

  println!("Indexes created successfully!");

  let indexes = repo.list_indexes().await?;
  println!("\nUser indexes: {:?}", indexes);

  repo.drop_index("age_1_name_-1").await?;
  println!("\nDropped index 'age_1_name_-1'");

  let indexes = repo.list_indexes().await?;
  println!("Indexes after drop: {:?}", indexes);

  let manager = IndexManager::new(provider);
  manager
    .create_single_field_index("users", "email", 1, false)
    .await?;
  println!("\nCreated email index via IndexManager");

  let info = manager.list_indexes("users").await?;
  println!("Indexes for users collection: {:?}", info);

  println!("\n=== Done ===");
  Ok(())
}
