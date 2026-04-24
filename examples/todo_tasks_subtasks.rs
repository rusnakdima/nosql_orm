//! Demonstrates 3-level relation loading: Todo -> Tasks -> Subtasks
//!
//! This example shows:
//! - Todo contains multiple Tasks (OneToMany: Task has todo_id)
//! - Each Task contains multiple Subtasks (ManyToMany: Task has subtask_ids)
//! - Relations are loaded declaratively via WithRelations impl
//!
//! Run: `cargo run --example todo_tasks_subtasks`

use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

/// Subtask entity - represents a subtask belonging to a Task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
  pub id: Option<String>,
  pub task_id: Option<String>,
  pub subtask_title: String,
  pub subtask_description: String,
  pub subtask_status: String,
  pub subtask_priority: u32,
  pub comments: Option<Vec<String>>,
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
    vec![RelationDef::many_to_one("task", "tasks", "task_id")]
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

/// Task entity - represents a task belonging to a Todo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
  pub id: Option<String>,
  pub todo_id: Option<String>,
  pub task_title: String,
  pub task_description: String,
  pub task_status: String,
  pub subtask_ids: Vec<String>,
  pub comments: Option<Vec<String>>,
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
      RelationDef::many_to_one("todo", "todos", "todo_id"),
      RelationDef::many_to_many("subtasks", "subtasks", "subtask_ids"),
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

/// Todo entity - top-level entity that contains multiple tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
  pub id: Option<String>,
  pub todo_title: String,
  pub todo_description: String,
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
    vec![RelationDef::one_to_many("tasks", "tasks", "todo_id")]
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

#[tokio::main]
async fn main() -> OrmResult<()> {
  let tmp = tempfile::tempdir().unwrap();
  let provider = JsonProvider::new(tmp.path()).await?;

  // Register relations for nested path loading to work
  register_collection_relations("todos", Todo::relations());
  register_collection_relations("tasks", Task::relations());
  register_collection_relations("subtasks", Subtask::relations());

  let subtasks_repo: Repository<Subtask, _> = Repository::new(provider.clone());
  let tasks_repo: RelationRepository<Task, _> = RelationRepository::new(provider.clone());
  let todos_repo: RelationRepository<Todo, _> = RelationRepository::new(provider.clone());

  // Create todo FIRST so we can associate tasks with it
  let todo = todos_repo
    .repo()
    .save(Todo {
      id: None,
      todo_title: "Build Rust ORM".to_string(),
      todo_description: "Build it".to_string(),
      deleted_at: None,
    })
    .await?;

  // Create subtasks
  let subtask1 = subtasks_repo
    .save(Subtask {
      id: None,
      task_id: None,
      subtask_title: "Setup structure".to_string(),
      subtask_description: "Create folders".to_string(),
      subtask_status: "completed".to_string(),
      subtask_priority: 1,
      comments: Some(vec!["Remember to create README".to_string()]),
      deleted_at: None,
    })
    .await?;
  let subtask2 = subtasks_repo
    .save(Subtask {
      id: None,
      task_id: None,
      subtask_title: "Write tests".to_string(),
      subtask_description: "Test all".to_string(),
      subtask_status: "in_progress".to_string(),
      subtask_priority: 2,
      comments: Some(vec!["Cover edge cases".to_string()]),
      deleted_at: None,
    })
    .await?;
  let subtask3 = subtasks_repo
    .save(Subtask {
      id: None,
      task_id: None,
      subtask_title: "Deploy".to_string(),
      subtask_description: "Deploy app".to_string(),
      subtask_status: "pending".to_string(),
      subtask_priority: 3,
      comments: None,
      deleted_at: None,
    })
    .await?;

  // Create tasks WITH todo_id set to associate with the todo
  let task1 = tasks_repo
    .repo()
    .save(Task {
      id: None,
      todo_id: todo.id.clone(),
      task_title: "Project Setup".to_string(),
      task_description: "Setup".to_string(),
      task_status: "completed".to_string(),
      subtask_ids: vec![subtask1.id.clone().unwrap()],
      comments: Some(vec!["Initial project structure".to_string()]),
      deleted_at: None,
    })
    .await?;
  let task2 = tasks_repo
    .repo()
    .save(Task {
      id: None,
      todo_id: todo.id.clone(),
      task_title: "Testing".to_string(),
      task_description: "Test".to_string(),
      task_status: "in_progress".to_string(),
      subtask_ids: vec![subtask2.id.clone().unwrap()],
      comments: Some(vec!["Need to add more tests".to_string()]),
      deleted_at: None,
    })
    .await?;
  let task3 = tasks_repo
    .repo()
    .save(Task {
      id: None,
      todo_id: todo.id.clone(),
      task_title: "Deployment".to_string(),
      task_description: "Deploy".to_string(),
      task_status: "pending".to_string(),
      subtask_ids: vec![subtask3.id.clone().unwrap()],
      comments: None,
      deleted_at: None,
    })
    .await?;

  println!("=== 1. Todo with Tasks (single level) ===\n");
  let todo_with_tasks = todos_repo
    .find_with_relations(todo.id.as_ref().unwrap(), &["tasks"])
    .await?
    .unwrap();
  let data = serde_json::to_string_pretty(&todo_with_tasks).unwrap();
  println!("{}\n", data);

  println!("=== 2. Task with Subtasks (single level) ===\n");
  let task_with_subtasks = tasks_repo
    .find_with_relations(task1.id.as_ref().unwrap(), &["subtasks"])
    .await?
    .unwrap();
  let data = serde_json::to_string_pretty(&task_with_subtasks).unwrap();
  println!("{}\n", data);

  println!("=== 3. Nested: Todo with Tasks AND Subtasks in each Task ===\n");
  let result = todos_repo
    .find_with_relations(todo.id.as_ref().unwrap(), &["tasks.subtasks"])
    .await?
    .unwrap();
  let data = serde_json::to_string_pretty(&result).unwrap();
  println!("{}\n", data);

  println!("=== 4. Find all Todos with nested Tasks and Subtasks ===\n");
  let all = todos_repo
    .find_all_with_relations(&["tasks.subtasks"])
    .await?;
  for r in &all {
    let data = serde_json::to_string_pretty(&r).unwrap();
    println!("{}\n", data);
  }

  println!("\n✓ Done!");
  Ok(())
}
