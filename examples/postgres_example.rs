//! PostgreSQL provider example
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
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

#[tokio::main]
async fn main() -> OrmResult<()> {
  let provider = PostgresProvider::new("postgres://user:pass@localhost/db").await?;
  let repo = Repository::<User, _>::new(provider);

  let user = User {
    id: None,
    name: "Alice".into(),
    email: "alice@example.com".into(),
  };
  let saved = repo.save(user).await?;
  println!("Saved: {:?}", saved);

  Ok(())
}
