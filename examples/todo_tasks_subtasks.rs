//! Demonstrates 3-level relation loading: Todo -> Tasks -> Subtasks
//!
//! Run: `cargo run --example todo_tasks_subtasks`

use chrono::{DateTime, Utc};
use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subtask {
  pub id: Option<String>,
  pub subtask_title: String,
  pub subtask_description: String,
  pub subtask_status: String,
  pub subtask_priority: u32,
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Subtask {
  fn meta() -> EntityMeta { EntityMeta::new("subtasks") }
  fn get_id(&self) -> Option<String> { self.id.clone() }
  fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for Subtask {
  fn relations() -> Vec<RelationDef> { vec![] }
}
impl SoftDeletable for Subtask {
  fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
  fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) { self.deleted_at = d; }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
  pub id: Option<String>,
  pub task_title: String,
  pub task_description: String,
  pub task_status: String,
  pub subtask_ids: Vec<String>,
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Task {
  fn meta() -> EntityMeta { EntityMeta::new("tasks") }
  fn get_id(&self) -> Option<String> { self.id.clone() }
  fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for Task {
  fn relations() -> Vec<RelationDef> {
    vec![RelationDef::many_to_many("subtasks", "subtasks", "subtask_ids")]
  }
}
impl SoftDeletable for Task {
  fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
  fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) { self.deleted_at = d; }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
  pub id: Option<String>,
  pub todo_title: String,
  pub todo_description: String,
  pub task_ids: Vec<String>,
  pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Todo {
  fn meta() -> EntityMeta { EntityMeta::new("todos") }
  fn get_id(&self) -> Option<String> { self.id.clone() }
  fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for Todo {
  fn relations() -> Vec<RelationDef> {
    vec![RelationDef::many_to_many("tasks", "tasks", "task_ids")]
  }
}
impl SoftDeletable for Todo {
  fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
  fn set_deleted_at(&mut self, d: Option<DateTime<Utc>>) { self.deleted_at = d; }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
  let tmp = tempfile::tempdir().unwrap();
  let provider = JsonProvider::new(tmp.path()).await?;

  let subtasks_repo: Repository<Subtask, _> = Repository::new(provider.clone());
  let tasks_repo: RelationRepository<Task, _> = RelationRepository::new(provider.clone());
  let todos_repo: RelationRepository<Todo, _> = RelationRepository::new(provider.clone());

  register_collection_relations("subtasks", vec![]);
  register_collection_relations("tasks", Task::relations());
  register_collection_relations("todos", Todo::relations());

  let subtask1 = subtasks_repo.save(Subtask { id: None, subtask_title: "Setup structure".to_string(), subtask_description: "Create folders".to_string(), subtask_status: "completed".to_string(), subtask_priority: 1, deleted_at: None }).await?;
  let subtask2 = subtasks_repo.save(Subtask { id: None, subtask_title: "Write tests".to_string(), subtask_description: "Test all".to_string(), subtask_status: "in_progress".to_string(), subtask_priority: 2, deleted_at: None }).await?;
  let subtask3 = subtasks_repo.save(Subtask { id: None, subtask_title: "Deploy".to_string(), subtask_description: "Deploy app".to_string(), subtask_status: "pending".to_string(), subtask_priority: 3, deleted_at: None }).await?;

  let task1 = tasks_repo.save(Task { id: None, task_title: "Project Setup".to_string(), task_description: "Setup".to_string(), task_status: "completed".to_string(), subtask_ids: vec![subtask1.id.clone().unwrap()], deleted_at: None }).await?;
  let task2 = tasks_repo.save(Task { id: None, task_title: "Testing".to_string(), task_description: "Test".to_string(), task_status: "in_progress".to_string(), subtask_ids: vec![subtask2.id.clone().unwrap()], deleted_at: None }).await?;
  let task3 = tasks_repo.save(Task { id: None, task_title: "Deployment".to_string(), task_description: "Deploy".to_string(), task_status: "pending".to_string(), subtask_ids: vec![subtask3.id.clone().unwrap()], deleted_at: None }).await?;

  let todo = todos_repo.save(Todo { id: None, todo_title: "Build Rust ORM".to_string(), todo_description: "Build it".to_string(), task_ids: vec![task1.id.unwrap(), task2.id.unwrap(), task3.id.unwrap()], deleted_at: None }).await?;

  println!("=== find_with_nested with path: 'tasks.subtasks' ===\n");
  let result = todos_repo.find_with_nested(todo.id.as_ref().unwrap(), "tasks.subtasks").await?.unwrap();

  println!("Todo: {}", result.entity.todo_title);
  println!("Tasks: {}", result.many("tasks")?.len());
  println!("Subtasks: {}", result.many("subtasks")?.len());

  for st in result.many("subtasks")? {
    println!("  - {} ({})", st.get("subtask_title").and_then(|v| v.as_str()).unwrap_or("?"), st.get("subtask_status").and_then(|v| v.as_str()).unwrap_or("?"));
  }

  println!("\n=== find_all_with_nested ===\n");
  let all = todos_repo.find_all_with_nested("tasks.subtasks").await?;
  for r in &all {
    println!("{}: {} tasks, {} subtasks", r.entity.todo_title, r.many("tasks")?.len(), r.many("subtasks")?.len());
  }

  println!("\n✓ Done!");
  Ok(())
}