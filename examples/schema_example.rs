//! Demonstrates storing and loading SDUI schemas using nosql_orm JSON provider.
//!
//! Run: `cargo run --example schema_example`

use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct UiSchema {
  pub id: Option<String>,
  pub schema_version: String,
  pub name: String,
  pub app: AppConfig,
  pub pages: Vec<Page>,
  pub layouts: Vec<Layout>,
  pub components: Vec<ComponentDef>,
  pub shared_components: Vec<ComponentDef>,
  pub services: Vec<ServiceDef>,
  pub modules: Vec<ModuleDef>,
  pub i18n: I18nConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
  pub id: String,
  pub name: String,
  pub version: String,
  pub description: String,
  pub identifier: String,
  pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
  pub default_locale: String,
  pub supported_locales: Vec<String>,
  pub tailwind_preset: String,
  pub theme: String,
  pub themes: Vec<String>,
  pub color_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
  pub id: String,
  pub name: String,
  pub route: String,
  pub layout: String,
  pub meta: PageMeta,
  pub canvas_elements: Vec<CanvasElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMeta {
  pub title: String,
  pub icon: Option<String>,
  pub breadcrumb: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasElement {
  pub id: String,
  pub component_id: String,
  pub grid_position: GridPosition,
  pub classes: String,
  pub children: Vec<String>,
  pub data_binding: Option<DataBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPosition {
  pub column: i32,
  pub row: i32,
  pub col_span: i32,
  pub row_span: i32,
  pub col_start: Option<i32>,
  pub row_start: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataBinding {
  pub entity: String,
  pub field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
  pub id: String,
  pub name: String,
  pub slots: std::collections::HashMap<String, LayoutSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutSlot {
  pub name: String,
  pub elements: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDef {
  pub id: String,
  pub name: String,
  pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDef {
  pub id: String,
  pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDef {
  pub id: String,
  pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18nConfig {
  pub locales: std::collections::HashMap<String, LocaleMap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleMap {
  pub nav: std::collections::HashMap<String, String>,
  pub actions: std::collections::HashMap<String, String>,
  pub messages: std::collections::HashMap<String, String>,
}

impl Entity for UiSchema {
  fn meta() -> EntityMeta {
    EntityMeta::new("schemas")
  }
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }
  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }
}

impl WithRelations for UiSchema {
  fn relations() -> Vec<RelationDef> {
    vec![]
  }
}

fn create_example_schema() -> UiSchema {
  let mut pages = std::collections::HashMap::new();
  pages.insert(
    "main".to_string(),
    LayoutSlot {
      name: "main".to_string(),
      elements: vec![
        "header-1".to_string(),
        "welcome-card".to_string(),
        "stats-card".to_string(),
      ],
    },
  );

  let mut locales = std::collections::HashMap::new();
  let mut nav = std::collections::HashMap::new();
  nav.insert("dashboard".to_string(), "Dashboard".to_string());
  nav.insert("about".to_string(), "About".to_string());
  let mut actions = std::collections::HashMap::new();
  actions.insert("submit".to_string(), "Submit".to_string());
  actions.insert("cancel".to_string(), "Cancel".to_string());
  let mut messages = std::collections::HashMap::new();
  messages.insert(
    "welcome".to_string(),
    "Welcome to the Example SDUI App".to_string(),
  );
  locales.insert(
    "en".to_string(),
    LocaleMap {
      nav,
      actions,
      messages,
    },
  );

  UiSchema {
    schema_version: "1.0.0".to_string(),
    id: None,
    name: "Example Schema".to_string(),
    app: AppConfig {
      id: "example-app".to_string(),
      name: "Example SDUI App".to_string(),
      version: "1.0.0".to_string(),
      description: "Example schema-driven app using nosql_orm".to_string(),
      identifier: "com.example.sdui".to_string(),
      settings: AppSettings {
        default_locale: "en".to_string(),
        supported_locales: vec!["en".to_string()],
        tailwind_preset: "default".to_string(),
        theme: "default".to_string(),
        themes: vec![],
        color_mode: "system".to_string(),
      },
    },
    pages: vec![Page {
      id: "dashboard-page".to_string(),
      name: "Dashboard".to_string(),
      route: "/".to_string(),
      layout: "default".to_string(),
      meta: PageMeta {
        title: "Dashboard".to_string(),
        icon: Some("dashboard".to_string()),
        breadcrumb: vec!["Home".to_string()],
      },
      canvas_elements: vec![
        CanvasElement {
          id: "header-1".to_string(),
          component_id: "app-header".to_string(),
          grid_position: GridPosition {
            column: 0,
            row: 0,
            col_span: 12,
            row_span: 1,
            col_start: None,
            row_start: None,
          },
          classes: "".to_string(),
          children: vec![],
          data_binding: None,
        },
        CanvasElement {
          id: "welcome-card".to_string(),
          component_id: "app-card".to_string(),
          grid_position: GridPosition {
            column: 0,
            row: 1,
            col_span: 6,
            row_span: 1,
            col_start: None,
            row_start: None,
          },
          classes: "".to_string(),
          children: vec![],
          data_binding: None,
        },
        CanvasElement {
          id: "stats-card".to_string(),
          component_id: "app-card".to_string(),
          grid_position: GridPosition {
            column: 6,
            row: 1,
            col_span: 6,
            row_span: 1,
            col_start: None,
            row_start: None,
          },
          classes: "".to_string(),
          children: vec![],
          data_binding: None,
        },
      ],
    }],
    layouts: vec![Layout {
      id: "default".to_string(),
      name: "Default Layout".to_string(),
      slots: pages,
    }],
    components: vec![],
    shared_components: vec![],
    services: vec![],
    modules: vec![],
    i18n: I18nConfig { locales },
  }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
  let tmp = tempfile::tempdir().unwrap();
  let provider = JsonProvider::new(tmp.path()).await?;
  let schemas: Repository<UiSchema, _> = Repository::new(provider.clone());

  println!("=== Creating example schema ===");
  let schema = create_example_schema();
  println!("Schema: {} (id: {:?})", schema.name, schema.id);

  println!("\n=== Saving schema to nosql_orm ===");
  let saved = schemas.save(schema).await?;
  println!("Saved schema with id: {:?}", saved.id);

  println!("\n=== Loading schema from nosql_orm ===");
  let schema_id = saved.id.clone().unwrap_or_default();
  let loaded = schemas.find_by_id(&schema_id).await?;
  match loaded {
    Some(s) => {
      println!("Loaded: {} (version: {})", s.name, s.schema_version);
      println!("Pages: {}", s.pages.len());
      println!("Layouts: {}", s.layouts.len());
      for page in &s.pages {
        println!(
          "  - Page '{}' at route '{}' with {} elements",
          page.name,
          page.route,
          page.canvas_elements.len()
        );
      }
    }
    None => println!("Schema not found!"),
  }

  println!("\n=== Querying all schemas ===");
  let all = schemas.find_all().await?;
  println!("Total schemas: {}", all.len());
  for s in &all {
    println!("  - {} ({})", s.name, s.id.as_deref().unwrap_or("(no id)"));
  }

  println!("\n✓ Schema example completed successfully.");
  println!("Data stored at: {}/schemas.json", tmp.path().display());
  Ok(())
}
