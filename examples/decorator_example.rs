use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm_derive::Model;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("decorator_users")]
#[soft_delete]
#[timestamp]
pub struct DecoratorUser {
  pub id: Option<String>,
  pub name: String,
  pub email: String,
  pub age: u32,
  // These fields are automatically added by the attributes:
  // #[soft_delete] -> pub deleted_at: Option<DateTime<Utc>>
  // #[timestamp] -> pub created_at: Option<DateTime<Utc>>,
  //              -> pub updated_at: Option<DateTime<Utc>>
}

// Manual implementation of SoftDeletable for DecoratorUser
// In a real implementation, the Model derive would add this automatically
impl SoftDeletable for DecoratorUser {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    None // Placeholder - in real implementation this would be a field
  }
  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    // Placeholder - in real implementation this would set a field
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
#[table_name("decorator_posts")]
#[many_to_one("author", "DecoratorUser", "id")]
#[many_to_many("categories", "DecoratorCategory", "category_ids")]
#[soft_delete] // Needed for find_with_relations to work
pub struct DecoratorPost {
  pub id: Option<String>,
  pub title: String,
  pub body: String,
  pub author_id: String,                 // Maps to many_to_one relation
  pub category_ids: Vec<String>,         // Maps to many_to_many relation
  pub deleted_at: Option<DateTime<Utc>>, // Required for SoftDeletable
}

// Manual implementation of SoftDeletable for DecoratorPost
impl SoftDeletable for DecoratorPost {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
    self.deleted_at = deleted_at;
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Model)]
pub struct DecoratorCategory {
  pub id: Option<String>,
  pub name: String,
}

#[tokio::main]
async fn main() -> OrmResult<()> {
  let provider = JsonProvider::new("./decorator_data").await?;

  let user_repo: Repository<DecoratorUser, _> = Repository::new(provider.clone());
  let post_repo: Repository<DecoratorPost, _> = Repository::new(provider.clone());
  let post_relation_repo: RelationRepository<DecoratorPost, _> =
    RelationRepository::new(provider.clone());
  let category_repo: Repository<DecoratorCategory, _> = Repository::new(provider);

  // Create a user
  let user = user_repo
    .save(DecoratorUser {
      id: None,
      name: "John Doe".into(),
      email: "john@example.com".into(),
      age: 30,
    })
    .await?;

  println!("Created user: {:?}", user);

  // Create a category
  let category = category_repo
    .save(DecoratorCategory {
      id: None,
      name: "Rust".into(),
    })
    .await?;

  println!("Created category: {:?}", category);

  // Create a post with relations
  let post = post_repo
    .save(DecoratorPost {
      id: None,
      title: "Using Decorators".into(),
      body: "This post demonstrates decorator usage.".into(),
      author_id: user.id.clone().unwrap(),
      category_ids: vec![category.id.clone().unwrap()],
      deleted_at: None,
    })
    .await?;

  println!("Created post: {:?}", post);

  // Load post with relations
  let post_with_relations = post_relation_repo
    .find_with_relations(&post.id.unwrap(), &["author", "categories"])
    .await?
    .unwrap();

  println!("\nPost with relations:");
  println!("  Title: {}", post_with_relations.entity.title);
  println!(
    "  Author: {}",
    post_with_relations
      .one("author")?
      .map(|a| a["name"].as_str().unwrap_or(""))
      .unwrap_or("")
  );
  println!(
    "  Categories: {:?}",
    post_with_relations.many("categories")?
  );

  Ok(())
}
