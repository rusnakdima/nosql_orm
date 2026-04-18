use nosql_orm::prelude::*;
use nosql_orm::sql::{SqlColumnDef, SqlColumnType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
  pub id: Option<i32>,
  pub name: String,
  pub email: String,
  pub age: i32,
}

impl Entity for User {
  fn meta() -> EntityMeta {
    EntityMeta::new("users")
  }
  fn get_id(&self) -> Option<String> {
    self.id.map(|i| i.to_string())
  }
  fn set_id(&mut self, id: String) {
    self.id = id.parse().ok();
  }

  fn sql_columns() -> Vec<SqlColumnDef> {
    vec![
      SqlColumnDef::new("id", SqlColumnType::Serial).primary_key(),
      SqlColumnDef::new("name", SqlColumnType::VarChar(255)),
      SqlColumnDef::new("email", SqlColumnType::VarChar(255)).unique(),
      SqlColumnDef::new("age", SqlColumnType::Integer),
    ]
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
  pub id: Option<i32>,
  pub title: String,
  pub content: String,
  pub author_id: i32,
}

impl Entity for Post {
  fn meta() -> EntityMeta {
    EntityMeta::new("posts")
  }
  fn get_id(&self) -> Option<String> {
    self.id.map(|i| i.to_string())
  }
  fn set_id(&mut self, id: String) {
    self.id = id.parse().ok();
  }

  fn sql_columns() -> Vec<SqlColumnDef> {
    vec![
      SqlColumnDef::new("id", SqlColumnType::Serial).primary_key(),
      SqlColumnDef::new("title", SqlColumnType::Text),
      SqlColumnDef::new("content", SqlColumnType::Text),
      SqlColumnDef::new("author_id", SqlColumnType::Integer),
    ]
  }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
  println!("=== SQL Provider Example ===\n");

  #[cfg(feature = "sql-sqlite")]
  {
    let provider = SqliteProvider::connect("./data/sqlite.db").await?;
    println!("Connected to SQLite database");

    let repo: Repository<User, _> = Repository::new(provider);

    repo.sync_schema().await?;
    println!("Schema synced for User entity");

    let user = User {
      id: None,
      name: "Alice".to_string(),
      email: "alice@example.com".to_string(),
      age: 30,
    };

    let saved = repo.save(user).await?;
    println!("Saved user: {:?}", saved);

    let found = repo.find_by_id(saved.get_id().unwrap()).await?;
    println!("Found user: {:?}", found);

    let users = repo
      .query()
      .where_gt("age", serde_json::json!(25))
      .find()
      .await?;
    println!("Users older than 25: {:?}", users);
  }

  #[cfg(not(feature = "sql-sqlite"))]
  {
    println!("SQLite feature not enabled. Run with --features sql-sqlite");
  }

  println!("\n=== Done ===");
  Ok(())
}
