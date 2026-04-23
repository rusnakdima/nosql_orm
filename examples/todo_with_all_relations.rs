//! Demonstrates getting Todo with ALL relations (Tasks + Subtasks) in ONE call
//!
//! Run: `cargo run --example todo_with_all_relations`

use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

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

struct TodoFullLoader {
    provider: JsonProvider,
    subtasks_cache: HashMap<String, Subtask>,
    tasks_cache: HashMap<String, Task>,
}

impl TodoFullLoader {
    fn new(provider: JsonProvider) -> Self {
        Self {
            provider,
            subtasks_cache: HashMap::new(),
            tasks_cache: HashMap::new(),
        }
    }

    async fn load_subtasks_for_task(&mut self, subtask_ids: &[String]) -> OrmResult<Vec<Subtask>> {
        let mut result = Vec::new();
        for sid in subtask_ids {
            let sub_val = if let Some(subtask) = self.subtasks_cache.get(sid).cloned() {
                subtask
            } else if let Some(found) = self.provider.find_by_id("subtasks", sid).await? {
                let sub: Subtask = serde_json::from_value(found.clone()).map_err(OrmError::Serialization)?;
                self.subtasks_cache.insert(sid.clone(), sub.clone());
                sub
            } else {
                continue;
            };
            result.push(sub_val);
        }
        Ok(result)
    }

    async fn load_tasks_for_todo(&mut self, task_ids: &[String]) -> OrmResult<Vec<(Task, Vec<Subtask>)>> {
        let mut result = Vec::new();
        for tid in task_ids {
            let task_val = if let Some(task) = self.tasks_cache.get(tid).cloned() {
                task
            } else if let Some(found) = self.provider.find_by_id("tasks", tid).await? {
                let task: Task = serde_json::from_value(found.clone()).map_err(OrmError::Serialization)?;
                self.tasks_cache.insert(tid.clone(), task.clone());
                task
            } else {
                continue;
            };

            let subtasks = self.load_subtasks_for_task(&task_val.subtask_ids).await?;
            result.push((task_val, subtasks));
        }
        Ok(result)
    }

    async fn load_todo_with_everything(&mut self, todo_id: &str) -> OrmResult<Option<(Todo, Vec<(Task, Vec<Subtask>)>)>> {
        let todo_val = match self.provider.find_by_id("todos", todo_id).await? {
            Some(v) => v,
            None => return Ok(None),
        };

        let todo: Todo = serde_json::from_value(todo_val.clone()).map_err(OrmError::Serialization)?;
        let tasks_with_subtasks = self.load_tasks_for_todo(&todo.task_ids).await?;

        Ok(Some((todo, tasks_with_subtasks)))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SubtaskOutput {
    id: String,
    subtask_title: String,
    subtask_description: String,
    subtask_status: String,
    subtask_priority: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskWithSubtasks {
    id: String,
    task_title: String,
    task_description: String,
    task_status: String,
    subtasks: Vec<SubtaskOutput>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TodoWithAllRelations {
    id: String,
    todo_title: String,
    todo_description: String,
    tasks: Vec<TaskWithSubtasks>,
}

fn print_todo_full(todo: &Todo, tasks_with_subtasks: &[(Task, Vec<Subtask>)]) {
    let output = TodoWithAllRelations {
        id: todo.id.clone().unwrap_or_default(),
        todo_title: todo.todo_title.clone(),
        todo_description: todo.todo_description.clone(),
        tasks: tasks_with_subtasks.iter().map(|(task, subtasks)| TaskWithSubtasks {
            id: task.id.clone().unwrap_or_default(),
            task_title: task.task_title.clone(),
            task_description: task.task_description.clone(),
            task_status: task.task_status.clone(),
            subtasks: subtasks.iter().map(|s| SubtaskOutput {
                id: s.id.clone().unwrap_or_default(),
                subtask_title: s.subtask_title.clone(),
                subtask_description: s.subtask_description.clone(),
                subtask_status: s.subtask_status.clone(),
                subtask_priority: s.subtask_priority,
            }).collect(),
        }).collect(),
    };

    println!("\n=== FULL TODO OUTPUT (SINGLE STRUCTURE) ===\n");
    println!("{}", serde_json::to_string_pretty(&output).unwrap());

    println!("\n=== FORMATTED VIEW ===\n");
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  TODO: {}                                          ║", todo.todo_title);
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Description: {}  ║", todo.todo_description);
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  TASKS: {}                                                   ║", tasks_with_subtasks.len());
    println!("╚══════════════════════════════════════════════════════════════════╝");

    for (i, (task, subtasks)) in tasks_with_subtasks.iter().enumerate() {
        println!("\n  ┌─── TASK #{}: {} ─────────────────────────", i + 1, task.task_title);
        println!("  │   Status: {} | Subtasks: {}", task.task_status, subtasks.len());
        println!("  │   Description: {}", task.task_description);

        if subtasks.is_empty() {
            println!("  │   └── (no subtasks)");
        } else {
            println!("  │   └── SUBTASKS:");
            for subtask in subtasks {
                let check = if subtask.subtask_status == "completed" { "✓" } else { "○" };
                println!("  │       {} [{}] {} - priority: {}",
                    check,
                    subtask.subtask_status,
                    subtask.subtask_title,
                    subtask.subtask_priority
                );
            }
        }
    }

    println!("\n═══════════════════════════════════════════════════════════════════════");
    let total_subtasks: usize = tasks_with_subtasks.iter().map(|(_, s)| s.len()).sum();
    let completed: usize = tasks_with_subtasks.iter()
        .flat_map(|(_, s)| s.iter())
        .filter(|st| st.subtask_status == "completed")
        .count();
    println!("  Total Tasks: {} | Total Subtasks: {} | Completed: {}/{}",
        tasks_with_subtasks.len(), total_subtasks, completed, total_subtasks);
    println!("═══════════════════════════════════════════════════════════════════════\n");
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let tmp = tempfile::tempdir().unwrap();
    let provider = JsonProvider::new(tmp.path()).await?;

    let subtasks_repo: Repository<Subtask, _> = Repository::new(provider.clone());
    let tasks_repo: RelationRepository<Task, _> = RelationRepository::new(provider.clone());
    let todos_repo: RelationRepository<Todo, _> = RelationRepository::new(provider.clone());

    println!("=== Creating Todo -> Tasks -> Subtasks data ===\n");

    let subtasks_data = vec![
        ("Setup project structure", "Create folder structure", "completed", 1),
        ("Implement Entity trait", "Add Entity implementations", "completed", 2),
        ("Write unit tests", "Test all core functionality", "in_progress", 1),
        ("Setup CI/CD pipeline", "Configure GitHub Actions", "pending", 3),
        ("Write documentation", "Document all APIs", "pending", 2),
        ("Setup logging", "Add logging infrastructure", "completed", 2),
    ];

    let mut created_subtasks = Vec::new();
    for (title, desc, status, priority) in subtasks_data {
        let sub = subtasks_repo.save(Subtask {
            id: None,
            subtask_title: title.to_string(),
            subtask_description: desc.to_string(),
            subtask_status: status.to_string(),
            subtask_priority: priority,
            deleted_at: None,
        }).await?;
        created_subtasks.push(sub);
    }

    let tasks_data = vec![
        ("Project Setup", "Initial project setup", "in_progress", vec![0, 1, 5]),
        ("Testing Phase", "Write and run tests", "pending", vec![2]),
        ("Documentation", "Document everything", "pending", vec![4]),
        ("Deployment", "Setup deployment", "pending", vec![3]),
    ];

    let mut created_tasks = Vec::new();
    for (title, desc, status, subtask_indices) in tasks_data {
        let subtask_ids: Vec<String> = subtask_indices.iter()
            .map(|&idx| created_subtasks[idx].id.clone().unwrap())
            .collect();

        let task = tasks_repo.save(Task {
            id: None,
            task_title: title.to_string(),
            task_description: desc.to_string(),
            task_status: status.to_string(),
            subtask_ids,
            deleted_at: None,
        }).await?;
        created_tasks.push(task);
    }

    let task_ids: Vec<String> = created_tasks.iter()
        .filter_map(|t| t.id.clone())
        .collect();

    let todo = todos_repo.save(Todo {
        id: None,
        todo_title: "Complete Rust ORM Implementation".to_string(),
        todo_description: "Build TypeORM-inspired ORM for NoSQL databases".to_string(),
        task_ids: task_ids.clone(),
        deleted_at: None,
    }).await?;

    println!("Created Todo: '{}' with {} tasks", todo.todo_title, created_tasks.len());
    println!("Created {} subtasks total\n", created_subtasks.len());

    println!("=== LOADING TODO #{} WITH ALL RELATIONS (ONE CALL) ===\n", &todo.id.as_ref().unwrap()[..8]);

    let mut loader = TodoFullLoader::new(provider.clone());
    if let Some((loaded_todo, tasks_with_subtasks)) = loader.load_todo_with_everything(todo.id.as_ref().unwrap()).await? {
        print_todo_full(&loaded_todo, &tasks_with_subtasks);
    }

    println!("\n✓ Todo with all relations loaded successfully!");

    let todos_file = tmp.path().join("todos.json");
    if todos_file.exists() {
        let content = tokio::fs::read_to_string(&todos_file).await?;
        println!("\n--- Stored JSON (snake_case) ---");
        println!("{}", content);
    }

    Ok(())
}