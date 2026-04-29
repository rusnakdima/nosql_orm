//! Demonstrates a TaskFlow-like data model with nosql_orm
//! Shows Todo -> Task -> Subtask -> Comment relations with cascade delete
//!
//! Run: `cargo run --example taskflow_like`

use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use nosql_orm::CascadeManager;
use nosql_orm::Validate;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct User {
  pub id: Option<String>,
  pub username: String,
  pub email: String,
  pub password: String,
  pub secret: String,
  pub created_at: Option<DateTime<Utc>>,
  pub updated_at: Option<DateTime<Utc>>,
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
    vec![]
  }
}

impl SoftDeletable for User {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) {
    self.deleted_at = d;
  }
}

impl FrontendProjection for User {
  fn frontend_excluded_fields() -> Vec<&'static str> {
    vec!["password", "secret"]
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Todo {
  pub id: Option<String>,
  pub user_id: String,
  pub title: String,
  pub description: String,
  pub visibility: String,
  pub priority: String,
  pub order: i32,
  pub created_at: Option<DateTime<Utc>>,
  pub updated_at: Option<DateTime<Utc>>,
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Todo {
  fn meta() -> EntityMeta {
    EntityMeta::new("todos")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

impl WithRelations for Todo {
  fn relations() -> Vec<RelationDef> {
    vec![
      RelationDef::one_to_many("tasks", "tasks", "todo_id")
        .on_delete(nosql_orm::sql::types::SqlOnDelete::Cascade),
      RelationDef::many_to_one("user", "users", "user_id"),
    ]
  }
}

impl SoftDeletable for Todo {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) {
    self.deleted_at = d;
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Task {
  pub id: Option<String>,
  pub todo_id: String,
  pub title: String,
  pub description: String,
  pub status: String,
  pub priority: String,
  pub order: i32,
  pub created_at: Option<DateTime<Utc>>,
  pub updated_at: Option<DateTime<Utc>>,
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Task {
  fn meta() -> EntityMeta {
    EntityMeta::new("tasks")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

impl WithRelations for Task {
  fn relations() -> Vec<RelationDef> {
    vec![
      RelationDef::one_to_many("subtasks", "subtasks", "task_id")
        .on_delete(nosql_orm::sql::types::SqlOnDelete::Cascade),
      RelationDef::one_to_many("comments", "comments", "task_id")
        .on_delete(nosql_orm::sql::types::SqlOnDelete::Cascade),
      RelationDef::many_to_one("todo", "todos", "todo_id"),
    ]
  }
}

impl SoftDeletable for Task {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) {
    self.deleted_at = d;
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Subtask {
  pub id: Option<String>,
  pub task_id: String,
  pub title: String,
  pub description: String,
  pub status: String,
  pub priority: String,
  pub order: i32,
  pub created_at: Option<DateTime<Utc>>,
  pub updated_at: Option<DateTime<Utc>>,
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Subtask {
  fn meta() -> EntityMeta {
    EntityMeta::new("subtasks")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

impl WithRelations for Subtask {
  fn relations() -> Vec<RelationDef> {
    vec![
      RelationDef::one_to_many("comments", "comments", "subtask_id")
        .on_delete(nosql_orm::sql::types::SqlOnDelete::Cascade),
      RelationDef::many_to_one("task", "tasks", "task_id"),
    ]
  }
}

impl SoftDeletable for Subtask {
  fn deleted_at(&self) -> Option<DateTime<Utc>> {
    self.deleted_at
  }
  fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) {
    self.deleted_at = d;
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Comment {
  pub id: Option<String>,
  pub author_id: String,
  pub author_name: String,
  pub content: String,
  pub task_id: Option<String>,
  pub subtask_id: Option<String>,
  pub read_by: Vec<String>,
  pub created_at: Option<DateTime<Utc>>,
  pub updated_at: Option<DateTime<Utc>>,
}

impl Entity for Comment {
  fn meta() -> EntityMeta {
    EntityMeta::new("comments")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

impl WithRelations for Comment {
  fn relations() -> Vec<RelationDef> {
    vec![
      RelationDef::many_to_one("task", "tasks", "task_id"),
      RelationDef::many_to_one("subtask", "subtasks", "subtask_id"),
    ]
  }
}

fn security_projection() -> Projection {
  Projection::exclude(&["password", "secret"])
}

#[allow(dead_code)]
fn apply_security_projection(docs: Vec<Value>) -> Vec<Value> {
  let projection = security_projection();
  docs
    .into_iter()
    .map(|doc| projection.apply_recursive(&doc))
    .collect()
}

#[tokio::main]
async fn main() -> OrmResult<()> {
  let tmp = tempfile::tempdir().unwrap();
  let provider = JsonProvider::new(tmp.path()).await?;

  let users_repo: Repository<User, _> = Repository::new(provider.clone());
  let todos_repo: RelationRepository<Todo, _> = RelationRepository::new(provider.clone());
  let tasks_repo: RelationRepository<Task, _> = RelationRepository::new(provider.clone());
  let subtasks_repo: RelationRepository<Subtask, _> = RelationRepository::new(provider.clone());
  let comments_repo: RelationRepository<Comment, _> = RelationRepository::new(provider.clone());

  println!("=== Creating TaskFlow-like data ===\n");

  let user = users_repo
    .save(User {
      id: None,
      username: "john_doe".into(),
      email: "john@example.com".into(),
      password: "hashed_password_123".into(),
      secret: "my_secret_key".into(),
      created_at: None,
      updated_at: None,
      deleted_at: None,
    })
    .await?;
  println!("Created user: {} (id: {:?})", user.username, user.id);

  let todo = todos_repo
    .repo()
    .save(Todo {
      id: None,
      user_id: user.id.clone().unwrap(),
      title: "Build TaskFlow App".into(),
      description: "Create a TaskFlow-like application with Rust".into(),
      visibility: "private".into(),
      priority: "high".into(),
      order: 1,
      created_at: None,
      updated_at: None,
      deleted_at: None,
    })
    .await?;
  println!("Created todo: {} (id: {:?})", todo.title, todo.id);

  let task1 = tasks_repo
    .repo()
    .save(Task {
      id: None,
      todo_id: todo.id.clone().unwrap(),
      title: "Design Database Schema".into(),
      description: "Create entity relationships".into(),
      status: "completed".into(),
      priority: "high".into(),
      order: 1,
      created_at: None,
      updated_at: None,
      deleted_at: None,
    })
    .await?;
  println!("Created task: {} (id: {:?})", task1.title, task1.id);

  let task2 = tasks_repo
    .repo()
    .save(Task {
      id: None,
      todo_id: todo.id.clone().unwrap(),
      title: "Implement API".into(),
      description: "Create REST endpoints".into(),
      status: "in_progress".into(),
      priority: "medium".into(),
      order: 2,
      created_at: None,
      updated_at: None,
      deleted_at: None,
    })
    .await?;
  println!("Created task: {} (id: {:?})", task2.title, task2.id);

  let subtask1 = subtasks_repo
    .repo()
    .save(Subtask {
      id: None,
      task_id: task1.id.clone().unwrap(),
      title: "Define entities".into(),
      description: "Create User, Todo, Task entities".into(),
      status: "completed".into(),
      priority: "high".into(),
      order: 1,
      created_at: None,
      updated_at: None,
      deleted_at: None,
    })
    .await?;
  println!(
    "Created subtask: {} (id: {:?})",
    subtask1.title, subtask1.id
  );

  let subtask2 = subtasks_repo
    .repo()
    .save(Subtask {
      id: None,
      task_id: task1.id.clone().unwrap(),
      title: "Add relations".into(),
      description: "Setup one_to_many, many_to_one".into(),
      status: "completed".into(),
      priority: "high".into(),
      order: 2,
      created_at: None,
      updated_at: None,
      deleted_at: None,
    })
    .await?;
  println!(
    "Created subtask: {} (id: {:?})",
    subtask2.title, subtask2.id
  );

  let comment1 = comments_repo
    .repo()
    .save(Comment {
      id: None,
      author_id: user.id.clone().unwrap(),
      author_name: user.username.clone(),
      content: "Great progress on the schema!".into(),
      task_id: Some(task1.id.clone().unwrap()),
      subtask_id: None,
      read_by: vec![],
      created_at: None,
      updated_at: None,
    })
    .await?;
  println!(
    "Created comment: {} (id: {:?})",
    comment1.content, comment1.id
  );

  println!("\n=== Demonstrating Security Projection (excludes password/secret) ===\n");

  let all_users_raw = users_repo.find_all().await?;
  let all_users = apply_security_projection(
    all_users_raw
      .into_iter()
      .map(|u| u.to_value().unwrap())
      .collect(),
  );
  println!("Users with password/secret excluded:");
  for user_doc in &all_users {
    println!(
      "  - username: {:?}",
      user_doc.get("username").and_then(|v| v.as_str())
    );
    println!(
      "    password: {:?} (should be None)",
      user_doc.get("password")
    );
    println!("    secret: {:?} (should be None)", user_doc.get("secret"));
  }

  println!("\n=== Demonstrating Relation Loading ===\n");

  let todo_with_tasks = todos_repo
    .find_with_relations(todo.id.as_ref().unwrap(), &["tasks"])
    .await?
    .unwrap();
  println!("Todo: {}", todo_with_tasks.entity.title);
  println!("  Tasks:");
  for task_val in todo_with_tasks.many("tasks")? {
    println!("    - {} [{}]", task_val["title"], task_val["status"]);
  }

  println!("\n=== Demonstrating Soft Delete with Cascade ===\n");

  let todo_id = todo.id.as_ref().unwrap();
  println!(
    "Soft-deleting todo '{}' (cascades to tasks, subtasks, comments)...",
    todo.title
  );

  todos_repo.soft_delete_cascade(todo_id).await?;
  println!("Todo soft-deleted successfully");

  let _restored = todos_repo.repo().restore(todo_id).await?;
  println!("Todo restored successfully");

  println!("\n=== Demonstrating Hard Delete Cascade ===\n");

  let cascade_manager = CascadeManager::new(provider.clone());
  let mut deleted_ids = std::collections::HashSet::new();

  let _cascade_deleted = cascade_manager
    .hard_delete_cascade::<Task>(
      task2.id.as_ref().unwrap(),
      &Task::relations(),
      &mut deleted_ids,
    )
    .await?;
  println!(
    "Hard delete cascade result: {} entities deleted",
    deleted_ids.len()
  );
  for id in &deleted_ids {
    println!("  - {}", id);
  }

  println!("\n=== Final Data State ===\n");

  let remaining_todos = todos_repo.repo().find_all().await?;
  let remaining_tasks = tasks_repo.repo().find_all().await?;
  let remaining_subtasks = subtasks_repo.repo().find_all().await?;
  let remaining_comments = comments_repo.repo().find_all().await?;

  println!("Todos: {}", remaining_todos.len());
  println!("Tasks: {}", remaining_tasks.len());
  println!("Subtasks: {}", remaining_subtasks.len());
  println!("Comments: {}", remaining_comments.len());

  println!("\n✓ TaskFlow-like example completed successfully!");
  Ok(())
}
