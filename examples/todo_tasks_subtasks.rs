//! Demonstrates 3-level relation loading: Todo -> Tasks -> Subtasks
//!
//! Run: `cargo run --example todo_tasks_subtasks`

use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

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

impl WithRelations for Subtask { fn relations() -> Vec<RelationDef> { vec![] } }
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

    println!("=== Creating 3-level hierarchy: Todo -> Tasks -> Subtasks ===\n");

    let subtask1 = subtasks_repo.save(Subtask {
        id: None,
        subtask_title: "Setup project structure".to_string(),
        subtask_description: "Create folder structure and config files".to_string(),
        subtask_status: "completed".to_string(),
        subtask_priority: 1,
        deleted_at: None,
    }).await?;

    let subtask2 = subtasks_repo.save(Subtask {
        id: None,
        subtask_title: "Implement Entity trait".to_string(),
        subtask_description: "Add Entity implementations".to_string(),
        subtask_status: "completed".to_string(),
        subtask_priority: 2,
        deleted_at: None,
    }).await?;

    let subtask3 = subtasks_repo.save(Subtask {
        id: None,
        subtask_title: "Write unit tests".to_string(),
        subtask_description: "Test all core functionality".to_string(),
        subtask_status: "in_progress".to_string(),
        subtask_priority: 1,
        deleted_at: None,
    }).await?;

    let subtask4 = subtasks_repo.save(Subtask {
        id: None,
        subtask_title: "Setup CI/CD pipeline".to_string(),
        subtask_description: "Configure GitHub Actions".to_string(),
        subtask_status: "pending".to_string(),
        subtask_priority: 3,
        deleted_at: None,
    }).await?;

    let task1 = tasks_repo.save(Task {
        id: None,
        task_title: "Project Setup".to_string(),
        task_description: "Initial project setup".to_string(),
        task_status: "in_progress".to_string(),
        subtask_ids: vec![subtask1.id.clone().unwrap(), subtask2.id.clone().unwrap()],
        deleted_at: None,
    }).await?;

    let task2 = tasks_repo.save(Task {
        id: None,
        task_title: "Testing Phase".to_string(),
        task_description: "Write and run tests".to_string(),
        task_status: "pending".to_string(),
        subtask_ids: vec![subtask3.id.clone().unwrap()],
        deleted_at: None,
    }).await?;

    let task3 = tasks_repo.save(Task {
        id: None,
        task_title: "Deployment".to_string(),
        task_description: "Setup deployment pipeline".to_string(),
        task_status: "pending".to_string(),
        subtask_ids: vec![subtask4.id.clone().unwrap()],
        deleted_at: None,
    }).await?;

    let todo = todos_repo.save(Todo {
        id: None,
        todo_title: "Complete Rust ORM Implementation".to_string(),
        todo_description: "Build a TypeORM-inspired ORM".to_string(),
        task_ids: vec![task1.id.clone().unwrap(), task2.id.clone().unwrap(), task3.id.clone().unwrap()],
        deleted_at: None,
    }).await?;

    println!("Created Todo: '{}' (id: {})", todo.todo_title, todo.id.clone().unwrap());
    println!("  Contains {} tasks: [{}]", todo.task_ids.len(),
        todo.task_ids.iter().map(|s| s[..8].to_string()).collect::<Vec<_>>().join(", "));

    println!("\n=== LEVEL 1: Load Todo with Tasks ===\n");

    let todo_with_tasks = todos_repo
        .find_with_relations(todo.id.as_ref().unwrap(), &["tasks"])
        .await?
        .unwrap();

    println!("Todo: {}", todo_with_tasks.entity.todo_title);

    let tasks_slice = todo_with_tasks.many("tasks")?;
    println!("  Tasks: {}", tasks_slice.len());

    for task_val in tasks_slice {
        let task_title = task_val.get("task_title").and_then(|v| v.as_str()).unwrap_or("?");
        let task_status = task_val.get("task_status").and_then(|v| v.as_str()).unwrap_or("?");
        let subtask_ids_count = task_val.get("subtask_ids").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
        println!("    - '{}' (status: {}, subtasks: {})", task_title, task_status, subtask_ids_count);
    }

    println!("\n=== LEVEL 2: Load each Task with its Subtasks ===\n");

    for task_val in tasks_slice {
        let task_id = task_val.get("id").and_then(|v| v.as_str()).unwrap_or("?");

        let task_with_subtasks = tasks_repo
            .find_with_relations(task_id, &["subtasks"])
            .await?
            .unwrap();

        println!("Task: {}", task_with_subtasks.entity.task_title);

        let subtasks_slice = task_with_subtasks.many("subtasks")?;
        for subtask_val in subtasks_slice {
            let title = subtask_val.get("subtask_title").and_then(|v| v.as_str()).unwrap_or("?");
            let status = subtask_val.get("subtask_status").and_then(|v| v.as_str()).unwrap_or("?");
            let priority = subtask_val.get("subtask_priority").and_then(|v| v.as_u64()).unwrap_or(0);
            println!("    - '{}' (status: {}, priority: {})", title, status, priority);
        }
    }

    println!("\n=== LEVEL 3: Full recursive chain via find_all ===\n");

    let all_todos = todos_repo.find_all_with_relations(&["tasks"]).await?;

    for todo_item in &all_todos {
        println!("\n=== Todo: '{}' ===", todo_item.entity.todo_title);

        let all_tasks = todo_item.many("tasks")?;
        println!("Tasks: {}", all_tasks.len());

        for task_val in all_tasks {
            let task_title = task_val.get("task_title").and_then(|v| v.as_str()).unwrap_or("?");
            let task_id = task_val.get("id").and_then(|v| v.as_str()).unwrap_or("");
            println!("  Task: '{}'", task_title);

            if let Ok(Some(task_entity)) = tasks_repo.repo().find_by_id(task_id).await {
                let subtask_ids = task_entity.subtask_ids.clone();
                for subtask_id in &subtask_ids {
                    if let Ok(Some(subtask)) = subtasks_repo.find_by_id(subtask_id).await {
                        println!("    Subtask: '{}' (priority: {}, status: {})",
                            subtask.subtask_title, subtask.subtask_priority, subtask.subtask_status);
                    }
                }
            }
        }
    }

    println!("\n=== JSON Storage Verification ===\n");

    let todos_file = tmp.path().join("todos.json");
    if todos_file.exists() {
        let content = tokio::fs::read_to_string(&todos_file).await?;
        println!("todos.json (snake_case):");
        println!("{}", content);
    }

    let tasks_file = tmp.path().join("tasks.json");
    if tasks_file.exists() {
        let content = tokio::fs::read_to_string(&tasks_file).await?;
        println!("\ntasks.json (snake_case):");
        println!("{}", content);
    }

    let subtasks_file = tmp.path().join("subtasks.json");
    if subtasks_file.exists() {
        let content = tokio::fs::read_to_string(&subtasks_file).await?;
        println!("\nsubtasks.json (snake_case):");
        println!("{}", content);
    }

    println!("\n✓ Todo -> Tasks -> Subtasks (3 levels) completed successfully!");
    Ok(())
}