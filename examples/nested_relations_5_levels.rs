//! Demonstrates 5 levels of nested relations
//!
//! Level 1: Post -> author_id -> User
//! Level 2: User -> profile_id -> Profile
//! Level 3: Profile -> company_id -> Company
//! Level 4: Company -> department_id -> Department
//! Level 5: Department -> manager_id -> Manager
//!
//! Run: `cargo run --example nested_relations_5_levels`

use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manager {
    pub id: Option<String>,
    pub manager_name: String,
    pub email: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Manager {
    fn meta() -> EntityMeta {
        EntityMeta::new("managers")
    }
    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}

impl WithRelations for Manager {
    fn relations() -> Vec<RelationDef> {
        vec![]
    }
}

impl SoftDeletable for Manager {
    fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
        self.deleted_at = deleted_at;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Department {
    pub id: Option<String>,
    pub department_name: String,
    pub manager_id: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Department {
    fn meta() -> EntityMeta {
        EntityMeta::new("departments")
    }
    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}

impl WithRelations for Department {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::many_to_one("manager", "managers", "manager_id"),
        ]
    }
}

impl SoftDeletable for Department {
    fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
        self.deleted_at = deleted_at;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub id: Option<String>,
    pub company_name: String,
    pub department_id: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Company {
    fn meta() -> EntityMeta {
        EntityMeta::new("companies")
    }
    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}

impl WithRelations for Company {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::many_to_one("department", "departments", "department_id"),
        ]
    }
}

impl SoftDeletable for Company {
    fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
        self.deleted_at = deleted_at;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: Option<String>,
    pub bio: String,
    pub avatar_url: String,
    pub company_id: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Profile {
    fn meta() -> EntityMeta {
        EntityMeta::new("profiles")
    }
    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}

impl WithRelations for Profile {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::many_to_one("company", "companies", "company_id"),
        ]
    }
}

impl SoftDeletable for Profile {
    fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
        self.deleted_at = deleted_at;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub user_name: String,
    pub email: String,
    pub profile_id: String,
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
        vec![
            RelationDef::many_to_one("profile", "profiles", "profile_id"),
        ]
    }
}

impl SoftDeletable for User {
    fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
        self.deleted_at = deleted_at;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub author_id: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Post {
    fn meta() -> EntityMeta {
        EntityMeta::new("posts")
    }
    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}

impl WithRelations for Post {
    fn relations() -> Vec<RelationDef> {
        vec![
            RelationDef::many_to_one("author", "users", "author_id"),
        ]
    }
}

impl SoftDeletable for Post {
    fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }
    fn set_deleted_at(&mut self, deleted_at: Option<DateTime<Utc>>) {
        self.deleted_at = deleted_at;
    }
}

#[tokio::main]
async fn main() -> OrmResult<()> {
    let tmp = tempfile::tempdir().unwrap();
    let provider = JsonProvider::new(tmp.path()).await?;

    let managers: Repository<Manager, _> = Repository::new(provider.clone());
    let departments: RelationRepository<Department, _> = RelationRepository::new(provider.clone());
    let companies: RelationRepository<Company, _> = RelationRepository::new(provider.clone());
    let profiles: RelationRepository<Profile, _> = RelationRepository::new(provider.clone());
    let users: RelationRepository<User, _> = RelationRepository::new(provider.clone());
    let posts: RelationRepository<Post, _> = RelationRepository::new(provider.clone());

    println!("=== Creating 5-level nested data ===\n");

    let manager = managers
        .save(Manager {
            id: None,
            manager_name: "John CEO".to_string(),
            email: "john@company.com".to_string(),
            deleted_at: None,
        })
        .await?;

    let department = departments
        .save(Department {
            id: None,
            department_name: "Engineering".to_string(),
            manager_id: manager.get_id().unwrap(),
            deleted_at: None,
        })
        .await?;

    let company = companies
        .save(Company {
            id: None,
            company_name: "Tech Corp".to_string(),
            department_id: department.get_id().unwrap(),
            deleted_at: None,
        })
        .await?;

    let profile = profiles
        .save(Profile {
            id: None,
            bio: "Senior Rust Developer".to_string(),
            avatar_url: "https://example.com/avatar.png".to_string(),
            company_id: company.get_id().unwrap(),
            deleted_at: None,
        })
        .await?;

    let user = users
        .save(User {
            id: None,
            user_name: "Alice Developer".to_string(),
            email: "alice@techcorp.com".to_string(),
            profile_id: profile.get_id().unwrap(),
            deleted_at: None,
        })
        .await?;

    let post = posts
        .save(Post {
            id: None,
            title: "Building nested relations in Rust ORM".to_string(),
            body: "This post demonstrates 5 levels of nested relations...".to_string(),
            author_id: user.get_id().unwrap(),
            deleted_at: None,
        })
        .await?;

    println!("Created hierarchy:");
    println!("  Level 5: Manager: {} ({})", manager.manager_name, manager.email);
    println!("  Level 4: Department: {} (manager: {})", department.department_name, department.manager_id);
    println!("  Level 3: Company: {} (department: {})", company.company_name, company.department_id);
    println!("  Level 2: Profile: {} (company: {})", profile.bio, profile.company_id);
    println!("  Level 1: User: {} (profile: {})", user.user_name, user.profile_id);
    println!("  Level 0: Post: '{}' by {}", post.title, post.author_id);

    println!("\n=== Loading relations level by level ===\n");

    println!("--- Level 0: Post -> author ---");
    let post_with_author = posts
        .find_with_relations(post.get_id().unwrap().as_str(), &["author"])
        .await?
        .unwrap();
    println!("Post: {}", post_with_author.entity.title);
    if let Some(author) = post_with_author.one("author")? {
        println!("  Author: {} (id: {})", author.get("user_name").unwrap(), author.get("id").unwrap());
    }

    println!("\n--- Level 1: User -> profile ---");
    let user_with_profile = users
        .find_with_relations(user.get_id().unwrap().as_str(), &["profile"])
        .await?
        .unwrap();
    println!("User: {}", user_with_profile.entity.user_name);
    if let Some(profile) = user_with_profile.one("profile")? {
        println!("  Profile bio: {}", profile.get("bio").unwrap());
    }

    println!("\n--- Level 2: Profile -> company ---");
    let profile_with_company = profiles
        .find_with_relations(profile.get_id().unwrap().as_str(), &["company"])
        .await?
        .unwrap();
    println!("Profile: {}", profile_with_company.entity.bio);
    if let Some(company) = profile_with_company.one("company")? {
        println!("  Company: {}", company.get("company_name").unwrap());
    }

    println!("\n--- Level 3: Company -> department ---");
    let company_with_dept = companies
        .find_with_relations(company.get_id().unwrap().as_str(), &["department"])
        .await?
        .unwrap();
    println!("Company: {}", company_with_dept.entity.company_name);
    if let Some(dept) = company_with_dept.one("department")? {
        println!("  Department: {}", dept.get("department_name").unwrap());
    }

    println!("\n--- Level 4: Department -> manager ---");
    let dept_with_manager = departments
        .find_with_relations(department.get_id().unwrap().as_str(), &["manager"])
        .await?
        .unwrap();
    println!("Department: {}", dept_with_manager.entity.department_name);
    if let Some(mgr) = dept_with_manager.one("manager")? {
        println!("  Manager: {} ({})", mgr.get("manager_name").unwrap(), mgr.get("email").unwrap());
    }

    println!("\n=== Loading multiple posts with full 5-level chain ===\n");

    let all_posts = posts.find_all_with_relations(&["author"]).await?;

    for post_item in &all_posts {
        println!("Post: '{}'", post_item.entity.title);

        if let Some(author_val) = post_item.one("author")? {
            let author_id = author_val.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            println!("  Level 1 - Author: id={}", author_id);

            if let Ok(Some(author_entity)) = users.repo().find_by_id(author_id).await {
                println!("    Level 1 Data: user_name={}, email={}",
                    author_entity.user_name,
                    author_entity.email
                );

                let profile_id = &author_entity.profile_id;
                println!("    Level 2 - Profile ID: {}", profile_id);

                if let Ok(Some(profile_entity)) = profiles.repo().find_by_id(profile_id).await {
                    println!("      Level 2 Data: bio={}", profile_entity.bio);

                    let company_id = &profile_entity.company_id;
                    println!("      Level 3 - Company ID: {}", company_id);

                    if let Ok(Some(company_entity)) = companies.repo().find_by_id(company_id).await {
                        println!("        Level 3 Data: company_name={}", company_entity.company_name);

                        let department_id = &company_entity.department_id;
                        println!("        Level 4 - Department ID: {}", department_id);

                        if let Ok(Some(dept_entity)) = departments.repo().find_by_id(department_id).await {
                            println!("          Level 4 Data: department_name={}", dept_entity.department_name);

                            let manager_id = &dept_entity.manager_id;
                            println!("          Level 5 - Manager ID: {}", manager_id);

                            if let Some(manager_entity) = managers.find_by_id(manager_id).await? {
                                println!("            Level 5 Data: manager_name={}, email={}",
                                    manager_entity.manager_name,
                                    manager_entity.email
                                );
                            }
                        }
                    }
                }
            }
        }
        println!();
    }

    println!("✓ 5-level nested relations test completed successfully!");
    Ok(())
}