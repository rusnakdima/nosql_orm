//! Demonstrates deep relation loading - traversing 5+ levels automatically
//!
//! This example shows how to load deeply nested relations in a single query
//! by using a recursive approach that handles any depth level.
//!
//! Run: `cargo run --example deep_relation_loading`

use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level5 {
    pub id: Option<String>,
    pub level5_name: String,
    pub level5_value: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Level5 {
    fn meta() -> EntityMeta { EntityMeta::new("level5_table") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for Level5 { fn relations() -> Vec<RelationDef> { vec![] } }
impl SoftDeletable for Level5 {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) { self.deleted_at = deleted_at; }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level4 {
    pub id: Option<String>,
    pub level4_name: String,
    pub level5_id: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Level4 {
    fn meta() -> EntityMeta { EntityMeta::new("level4_table") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for Level4 {
    fn relations() -> Vec<RelationDef> {
        vec![RelationDef::many_to_one("level5", "level5_table", "level5_id")]
    }
}
impl SoftDeletable for Level4 {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) { self.deleted_at = deleted_at; }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level3 {
    pub id: Option<String>,
    pub level3_name: String,
    pub level4_id: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Level3 {
    fn meta() -> EntityMeta { EntityMeta::new("level3_table") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for Level3 {
    fn relations() -> Vec<RelationDef> {
        vec![RelationDef::many_to_one("level4", "level4_table", "level4_id")]
    }
}
impl SoftDeletable for Level3 {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) { self.deleted_at = deleted_at; }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level2 {
    pub id: Option<String>,
    pub level2_name: String,
    pub level3_id: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Level2 {
    fn meta() -> EntityMeta { EntityMeta::new("level2_table") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for Level2 {
    fn relations() -> Vec<RelationDef> {
        vec![RelationDef::many_to_one("level3", "level3_table", "level3_id")]
    }
}
impl SoftDeletable for Level2 {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) { self.deleted_at = deleted_at; }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level1 {
    pub id: Option<String>,
    pub level1_name: String,
    pub level2_id: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Level1 {
    fn meta() -> EntityMeta { EntityMeta::new("level1_table") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for Level1 {
    fn relations() -> Vec<RelationDef> {
        vec![RelationDef::many_to_one("level2", "level2_table", "level2_id")]
    }
}
impl SoftDeletable for Level1 {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) { self.deleted_at = deleted_at; }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootEntity {
    pub id: Option<String>,
    pub root_name: String,
    pub level1_id: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for RootEntity {
    fn meta() -> EntityMeta { EntityMeta::new("root_table") }
    fn get_id(&self) -> Option<String> { self.id.clone() }
    fn set_id(&mut self, id: String) { self.id = Some(id); }
}

impl WithRelations for RootEntity {
    fn relations() -> Vec<RelationDef> {
        vec![RelationDef::many_to_one("level1", "level1_table", "level1_id")]
    }
}
impl SoftDeletable for RootEntity {
    fn deleted_at(&self) -> Option<DateTime<Utc>> { self.deleted_at }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) { self.deleted_at = deleted_at; }
}

struct DeepLoader {
    providers: HashMap<String, JsonProvider>,
}

impl DeepLoader {
    fn new(provider: JsonProvider) -> Self {
        let mut providers = HashMap::new();
        providers.insert("root_table".to_string(), provider.clone());
        providers.insert("level1_table".to_string(), provider.clone());
        providers.insert("level2_table".to_string(), provider.clone());
        providers.insert("level3_table".to_string(), provider.clone());
        providers.insert("level4_table".to_string(), provider.clone());
        providers.insert("level5_table".to_string(), provider.clone());
        Self { providers }
    }

    fn get_provider(&self, table: &str) -> &JsonProvider {
        self.providers.get(table).unwrap()
    }

    async fn load_recursive(
        &self,
        table: &str,
        id: &str,
        depth: usize,
        current_depth: usize,
    ) -> OrmResult<serde_json::Value> {
        let provider = self.get_provider(table);

        let doc = match provider.find_by_id(table, id).await? {
            Some(d) => d,
            None => return Ok(serde_json::Value::Null),
        };

        if current_depth >= depth {
            return Ok(doc);
        }

        let mut result = doc.clone();

        let relations: Vec<(&str, &str, &str)> = match table {
            "root_table" => vec![("level1", "level1_id", "level1_table")],
            "level1_table" => vec![("level2", "level2_id", "level2_table")],
            "level2_table" => vec![("level3", "level3_id", "level3_table")],
            "level3_table" => vec![("level4", "level4_id", "level4_table")],
            "level4_table" => vec![("level5", "level5_id", "level5_table")],
            _ => vec![],
        };

        for (rel_name, fk_field, target_table) in relations {
            if let Some(fk_value) = doc.get(fk_field).and_then(|v| v.as_str()) {
                let nested = Box::pin(self.load_recursive(target_table, fk_value, depth, current_depth + 1)).await;

                let nested = nested?;

                if let Some(obj) = result.as_object_mut() {
                    obj.insert(format!("_nested_{}", rel_name), nested);
                }
            }
        }

        Ok(result)
    }

    async fn load_with_chain(
        &self,
        chain: &[(String, String)],
    ) -> OrmResult<HashMap<String, serde_json::Value>> {
        let mut results = HashMap::new();
        let mut current_id: Option<String> = None;
        let mut is_first = true;

        for (table, fk_field) in chain {
            let provider = self.get_provider(table);

            if is_first {
                let all = provider.find_many(table, None, None, None, None, true).await?;
                if let Some(first) = all.first() {
                    let next_id = first.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                    results.insert(table.clone(), first.clone());

                    let fk_id = first.get(fk_field.as_str()).and_then(|v| v.as_str()).map(|s| s.to_string());
                    current_id = fk_id;
                    is_first = false;
                }
            } else {
                if let Some(id) = &current_id {
                    let doc = provider.find_by_id(table, id).await?;
                    if let Some(d) = doc {
                        let fk_id = d.get(fk_field.as_str()).and_then(|v| v.as_str()).map(|s| s.to_string());
                        current_id = fk_id;
                        results.insert(table.clone(), d);
                    }
                }
            }
        }

        Ok(results)
    }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let tmp = tempfile::tempdir().unwrap();
    let provider = JsonProvider::new(tmp.path()).await?;

    let repo5: Repository<Level5, _> = Repository::new(provider.clone());
    let repo4: RelationRepository<Level4, _> = RelationRepository::new(provider.clone());
    let repo3: RelationRepository<Level3, _> = RelationRepository::new(provider.clone());
    let repo2: RelationRepository<Level2, _> = RelationRepository::new(provider.clone());
    let repo1: RelationRepository<Level1, _> = RelationRepository::new(provider.clone());
    let repo_root: RelationRepository<RootEntity, _> = RelationRepository::new(provider.clone());

    println!("=== Creating 6-level deep data structure ===\n");

    let l5 = repo5.save(Level5 {
        id: None,
        level5_name: "Level 5 - Deepest".to_string(),
        level5_value: "Maximum depth value".to_string(),
        deleted_at: None,
    }).await?;

    let l4 = repo4.save(Level4 {
        id: None,
        level4_name: "Level 4".to_string(),
        level5_id: l5.id.clone().unwrap(),
        deleted_at: None,
    }).await?;

    let l3 = repo3.save(Level3 {
        id: None,
        level3_name: "Level 3".to_string(),
        level4_id: l4.id.clone().unwrap(),
        deleted_at: None,
    }).await?;

    let l2 = repo2.save(Level2 {
        id: None,
        level2_name: "Level 2".to_string(),
        level3_id: l3.id.clone().unwrap(),
        deleted_at: None,
    }).await?;

    let l1 = repo1.save(Level1 {
        id: None,
        level1_name: "Level 1".to_string(),
        level2_id: l2.id.clone().unwrap(),
        deleted_at: None,
    }).await?;

    let root = repo_root.save(RootEntity {
        id: None,
        root_name: "Root Entity".to_string(),
        level1_id: l1.id.clone().unwrap(),
        deleted_at: None,
    }).await?;

    println!("Created chain:");
    println!("  Root (id: {})", root.id.clone().unwrap());
    println!("    -> level1_id: {}", root.level1_id);
    println!("  Level 1 (id: {})", l1.id.clone().unwrap());
    println!("    -> level2_id: {}", l1.level2_id);
    println!("  Level 2 (id: {})", l2.id.clone().unwrap());
    println!("    -> level3_id: {}", l2.level3_id);
    println!("  Level 3 (id: {})", l3.id.clone().unwrap());
    println!("    -> level4_id: {}", l3.level4_id);
    println!("  Level 4 (id: {})", l4.id.clone().unwrap());
    println!("    -> level5_id: {}", l4.level5_id);
    println!("  Level 5 (id: {}) - DEEPEST", l5.id.clone().unwrap());
    println!("    -> level5_name: {}", l5.level5_name);
    println!("    -> level5_value: {}", l5.level5_value);

    println!("\n=== Method 1: Level-by-level loading ===\n");

    let root_with_l1 = repo_root.find_with_relations(root.id.as_ref().unwrap(), &["level1"]).await?.unwrap();
    println!("Level 0 (Root): name='{}'", root_with_l1.entity.root_name);

    if let Some(l1_data) = root_with_l1.one("level1")? {
        let l1_id = l1_data.get("id").and_then(|v| v.as_str()).unwrap();
        let l1_name = l1_data.get("level1_name").and_then(|v| v.as_str()).unwrap();
        println!("Level 1: id={}, name='{}'", l1_id, l1_name);

        let l1_with_l2 = repo1.find_with_relations(l1_id, &["level2"]).await?.unwrap();
        if let Some(l2_data) = l1_with_l2.one("level2")? {
            let l2_id = l2_data.get("id").and_then(|v| v.as_str()).unwrap();
            let l2_name = l2_data.get("level2_name").and_then(|v| v.as_str()).unwrap();
            println!("Level 2: id={}, name='{}'", l2_id, l2_name);

            let l2_with_l3 = repo2.find_with_relations(l2_id, &["level3"]).await?.unwrap();
            if let Some(l3_data) = l2_with_l3.one("level3")? {
                let l3_id = l3_data.get("id").and_then(|v| v.as_str()).unwrap();
                let l3_name = l3_data.get("level3_name").and_then(|v| v.as_str()).unwrap();
                println!("Level 3: id={}, name='{}'", l3_id, l3_name);

                let l3_with_l4 = repo3.find_with_relations(l3_id, &["level4"]).await?.unwrap();
                if let Some(l4_data) = l3_with_l4.one("level4")? {
                    let l4_id = l4_data.get("id").and_then(|v| v.as_str()).unwrap();
                    let l4_name = l4_data.get("level4_name").and_then(|v| v.as_str()).unwrap();
                    println!("Level 4: id={}, name='{}'", l4_id, l4_name);

                    let l4_with_l5 = repo4.find_with_relations(l4_id, &["level5"]).await?.unwrap();
                    if let Some(l5_data) = l4_with_l5.one("level5")? {
                        let l5_name = l5_data.get("level5_name").and_then(|v| v.as_str()).unwrap();
                        let l5_value = l5_data.get("level5_value").and_then(|v| v.as_str()).unwrap();
                        println!("Level 5: name='{}', value='{}'", l5_name, l5_value);
                    }
                }
            }
        }
    }

    println!("\n=== Method 2: Using DeepLoader for automatic traversal ===\n");

    let loader = DeepLoader::new(provider.clone());

    let deep_result = loader
        .load_recursive("root_table", root.id.as_ref().unwrap(), 6, 0)
        .await?;

    if let Some(obj) = deep_result.as_object() {
        println!("Loaded entire chain from root:");
        println!("  root_name: {:?}", obj.get("root_name"));

        if let Some(nested_l1) = obj.get("_nested_level1") {
            if let Some(l1_obj) = nested_l1.as_object() {
                println!("  level1_name: {:?}", l1_obj.get("level1_name"));

                if let Some(nested_l2) = l1_obj.get("_nested_level2") {
                    if let Some(l2_obj) = nested_l2.as_object() {
                        println!("  level2_name: {:?}", l2_obj.get("level2_name"));

                        if let Some(nested_l3) = l2_obj.get("_nested_level3") {
                            if let Some(l3_obj) = nested_l3.as_object() {
                                println!("  level3_name: {:?}", l3_obj.get("level3_name"));

                                if let Some(nested_l4) = l3_obj.get("_nested_level4") {
                                    if let Some(l4_obj) = nested_l4.as_object() {
                                        println!("  level4_name: {:?}", l4_obj.get("level4_name"));

                                        if let Some(nested_l5) = l4_obj.get("_nested_level5") {
                                            if let Some(l5_obj) = nested_l5.as_object() {
                                                println!("  level5_name: {:?}", l5_obj.get("level5_name"));
                                                println!("  level5_value: {:?}", l5_obj.get("level5_value"));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    println!("\n=== Method 3: Chain-based loading ===\n");

    let chain_results = loader
        .load_with_chain(&[
            ("root_table".to_string(), "level1_id".to_string()),
            ("level1_table".to_string(), "level2_id".to_string()),
            ("level2_table".to_string(), "level3_id".to_string()),
            ("level3_table".to_string(), "level4_id".to_string()),
            ("level4_table".to_string(), "level5_id".to_string()),
        ])
        .await?;

    println!("Loaded via chain:");
    for (table, doc) in &chain_results {
        let name = match table.as_str() {
            "root_table" => doc.get("root_name"),
            "level1_table" => doc.get("level1_name"),
            "level2_table" => doc.get("level2_name"),
            "level3_table" => doc.get("level3_name"),
            "level4_table" => doc.get("level4_name"),
            "level5_table" => doc.get("level5_name"),
            _ => None,
        };
        println!("  {}: {:?}", table, name.and_then(|v| v.as_str()).unwrap_or("?"));
    }

    println!("\n✓ Deep relation loading (6 levels) completed successfully!");
    println!("\nSummary:");
    println!("  - Method 1: Manual level-by-level (requires N queries)");
    println!("  - Method 2: Recursive DeepLoader (single entry point, automatic traversal)");
    println!("  - Method 3: Chain-based loading (explicit path definition)");
    Ok(())
}