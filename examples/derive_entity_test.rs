//! Test example for simplified Entity derive macro
//! Demonstrates how a starter can now define entities with minimal boilerplate

use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm_derive::Entity;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Entity)]
pub struct User {
  id: Option<String>,
  email: String,
  username: String,
  password: String,
  role: String,
  deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Entity)]
#[entity("profiles")]
pub struct Profile {
  id: Option<String>,
  user_id: String,
  display_name: String,
  avatar_url: Option<String>,
  bio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Entity)]
#[Relations(todos)]
pub struct Category {
  id: Option<String>,
  name: String,
  color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Entity)]
#[Relations(subtasks, comments)]
pub struct Task {
  id: Option<String>,
  title: String,
  description: String,
  status: String,
  priority: String,
  start_date: Option<DateTime<Utc>>,
  end_date: Option<DateTime<Utc>>,
  deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Entity)]
#[Relations(comments)]
pub struct Subtask {
  id: Option<String>,
  task_id: String,
  title: String,
  description: String,
  status: String,
  priority: String,
  order: i32,
  start_date: Option<DateTime<Utc>>,
  end_date: Option<DateTime<Utc>>,
  deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Entity)]
#[Relations(tasks)]
pub struct Todo {
  id: Option<String>,
  user_id: String,
  title: String,
  description: String,
  start_date: Option<DateTime<Utc>>,
  end_date: Option<DateTime<Utc>>,
  priority: String,
  visibility: String,
  order: i32,
  categories: Vec<String>,
  assignees: Vec<String>,
  deleted_at: Option<DateTime<Utc>>,
}

#[tokio::main]
async fn main() -> OrmResult<()> {
  println!("Testing simplified Entity derive macro...\n");

  let provider = JsonProvider::new("./test_data").await?;
  let repo: Repository<Todo, _> = Repository::new(provider.clone());
  let user_repo: Repository<User, _> = Repository::new(provider.clone());
  let profile_repo: Repository<Profile, _> = Repository::new(provider.clone());
  let task_repo: Repository<Task, _> = Repository::new(provider.clone());
  let subtask_repo: Repository<Subtask, _> = Repository::new(provider.clone());
  let category_repo: Repository<Category, _> = Repository::new(provider.clone());

  println!("✅ Repositories created successfully");

  println!("\n✅ User entity test:");
  println!("   User table: {}", User::table_name());
  println!("   User relations: {:?}", User::relations());

  println!("\n✅ Profile entity test:");
  println!("   Profile table: {}", Profile::table_name());

  println!("\n✅ Todo entity test:");
  println!("   Todo table: {}", Todo::table_name());

  println!("\n✅ Task entity test:");
  println!("   Task table: {}", Task::table_name());

  println!("\n✅ Subtask entity test:");
  println!("   Subtask table: {}", Subtask::table_name());

  println!("\n✅ Category entity test:");
  println!("   Category table: {}", Category::table_name());

  let user = User {
    id: None,
    email: "test@example.com".to_string(),
    username: "testuser".to_string(),
    password: "hashed".to_string(),
    role: "user".to_string(),
    deleted_at: None,
  };
  let saved_user = user_repo.save(user).await?;
  println!("\n✅ User saved: {:?}", saved_user.get_id());

  let saved_todo = repo
    .save(Todo {
      id: None,
      user_id: saved_user.get_id().unwrap(),
      title: "Test Todo".to_string(),
      description: "Description".to_string(),
      start_date: None,
      end_date: None,
      priority: "high".to_string(),
      visibility: "private".to_string(),
      order: 0,
      categories: vec![],
      assignees: vec![],
      deleted_at: None,
    })
    .await?;
  println!("✅ Todo saved: {:?}", saved_todo.get_id());

  println!("\n🎉 All tests passed! The simplified Entity derive macro works!");

  Ok(())
}
