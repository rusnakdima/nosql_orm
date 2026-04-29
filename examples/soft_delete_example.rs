use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm::SoftDeletable;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SoftDeletableUser {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for SoftDeletableUser {
  fn meta() -> EntityMeta {
    EntityMeta::new("soft_delete_users")
  }

  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }

  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }

  fn is_soft_deletable() -> bool {
    true
  }
}

impl WithRelations for SoftDeletableUser {
  fn relations() -> Vec<RelationDef> {
    vec![]
  }
}

impl SoftDeletable for SoftDeletableUser {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }

  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    self.deleted_at = deleted_at;
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let temp_dir = tempfile::tempdir()?;
  let provider = JsonProvider::new(temp_dir.path()).await?;
  let repo: Repository<SoftDeletableUser, _> = Repository::new(provider);

  let user = SoftDeletableUser {
    id: None,
    name: "Alice".into(),
    email: "alice@example.com".into(),
    deleted_at: None,
  };
  let saved = repo.save(user).await?;
  println!("Created user: {:?}", saved);
  let user_id = saved.get_id().unwrap();

  let all: Vec<SoftDeletableUser> = repo.find_all().await?;
  println!("Before soft delete: {} users", all.len());

  repo.soft_delete(&user_id).await?;
  println!("Soft deleted user: {}", user_id);

  let all_after: Vec<SoftDeletableUser> = repo.find_all().await?;
  println!("After soft delete (find_all): {} users", all_after.len());

  let all_including: Vec<SoftDeletableUser> = repo.find_all_including_deleted().await?;
  println!(
    "After soft delete (find_all_including_deleted): {} users",
    all_including.len()
  );

  repo.restore(&user_id).await?;
  println!("Restored user: {}", user_id);

  let all_restored: Vec<SoftDeletableUser> = repo.find_all().await?;
  println!("After restore: {} users", all_restored.len());

  let query_all: Vec<SoftDeletableUser> = repo.query().find().await?;
  println!("Query result: {} users", query_all.len());

  let query_including: Vec<SoftDeletableUser> = repo.query_including_deleted().find().await?;
  println!("Query including deleted: {} users", query_including.len());

  Ok(())
}
