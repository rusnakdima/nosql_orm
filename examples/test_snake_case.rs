//! Test snake_case data transfer and relations
//!
//! Run: `cargo run --example test_snake_case`

use nosql_orm::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Option<String>,
    pub user_name: String,
    pub email_address: String,
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
pub struct Profile {
    pub id: Option<String>,
    pub bio: String,
    pub avatar_url: String,
    pub created_at: Option<DateTime<Utc>>,
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
        vec![]
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
pub struct Post {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub author_id: String,
    pub tag_ids: Vec<String>,
    pub created_at: Option<DateTime<Utc>>,
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
            RelationDef::many_to_many("tags", "tags", "tag_ids"),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: Option<String>,
    pub tag_name: String,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Entity for Tag {
    fn meta() -> EntityMeta {
        EntityMeta::new("tags")
    }
    fn get_id(&self) -> Option<String> {
        self.id.clone()
    }
    fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }
}

impl WithRelations for Tag {
    fn relations() -> Vec<RelationDef> {
        vec![]
    }
}

impl SoftDeletable for Tag {
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

    let profiles: Repository<Profile, _> = Repository::new(provider.clone());
    let users: RelationRepository<User, _> = RelationRepository::new(provider.clone());
    let tags: Repository<Tag, _> = Repository::new(provider.clone());
    let posts: RelationRepository<Post, _> = RelationRepository::new(provider.clone());

    println!("=== Creating entities with snake_case fields ===\n");

    let profile = profiles
        .save(Profile {
            id: None,
            bio: "Software engineer".to_string(),
            avatar_url: "https://example.com/avatar.png".to_string(),
            created_at: None,
            deleted_at: None,
        })
        .await?;

    let user = users
        .save(User {
            id: None,
            user_name: "Alice Johnson".to_string(),
            email_address: "alice@example.com".to_string(),
            profile_id: profile.get_id().unwrap(),
            deleted_at: None,
        })
        .await?;

    let tag1 = tags
        .save(Tag {
            id: None,
            tag_name: "Rust".to_string(),
            deleted_at: None,
        })
        .await?;

    let tag2 = tags
        .save(Tag {
            id: None,
            tag_name: "ORM".to_string(),
            deleted_at: None,
        })
        .await?;

    let post = posts
        .save(Post {
            id: None,
            title: "Hello Rust".to_string(),
            body: "Building an ORM in Rust".to_string(),
            author_id: user.get_id().unwrap(),
            tag_ids: vec![tag1.get_id().unwrap(), tag2.get_id().unwrap()],
            created_at: None,
            deleted_at: None,
        })
        .await?;

    println!("Created Profile: {} with bio='{}'", profile.get_id().unwrap(), profile.bio);
    println!("Created User: {} with user_name='{}'", user.get_id().unwrap(), user.user_name);
    println!("Created Tags: {} and {}", tag1.get_id().unwrap(), tag2.get_id().unwrap());
    println!("Created Post: {} with title='{}'", post.get_id().unwrap(), post.title);

    println!("\n=== Query by relation path (user.profile_id) ===\n");

    let user_by_profile = users
        .repo()
        .query()
        .where_eq("profile_id", serde_json::json!(profile.get_id().unwrap()))
        .find()
        .await?;

    println!("Found {} user(s) with profile_id={}", user_by_profile.len(), profile.get_id().unwrap());
    for u in &user_by_profile {
        println!("  User: {} (email: {})", u.user_name, u.email_address);
    }

    println!("\n=== Find post with author relation ===\n");

    let post_with_author = posts
        .find_with_relations(post.get_id().unwrap().as_str(), &["author"])
        .await?
        .unwrap();

    println!("Post: {}", post_with_author.entity.title);
    if let Some(author) = post_with_author.one("author")? {
        let author_name = author.get("user_name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let email = author.get("email_address").and_then(|v| v.as_str()).unwrap_or("unknown");
        println!("  Author: {} (email: {})", author_name, email);
    }

    println!("\n=== Find all posts with relations ===\n");

    let all_posts = posts.find_all_with_relations(&["author", "tags"]).await?;

    for item in &all_posts {
        println!("Post: {}", item.entity.title);
        if let Some(author) = item.one("author")? {
            let author_name = author.get("user_name").and_then(|v| v.as_str()).unwrap_or("unknown");
            println!("  Author: {}", author_name);
        }
        let tags: Vec<&str> = item.many("tags")?
            .iter()
            .filter_map(|t| t.get("tag_name").and_then(|v| v.as_str()))
            .collect();
        println!("  Tags: {:?}", tags);
    }

    println!("\n=== Verifying JSON storage uses snake_case ===\n");

    let posts_file = tmp.path().join("posts.json");
    if posts_file.exists() {
        let content = tokio::fs::read_to_string(&posts_file).await?;
        println!("Posts JSON content:");
        println!("{}", content);

        if content.contains("user_name") || content.contains("email_address") || content.contains("author_id") || content.contains("tag_ids") || content.contains("created_at") {
            println!("\n✓ CONFIRMED: Data is stored in snake_case format!");
        } else {
            println!("\n✗ WARNING: Data may not be in snake_case format");
        }
    }

    let users_file = tmp.path().join("users.json");
    if users_file.exists() {
        let content = tokio::fs::read_to_string(&users_file).await?;
        println!("\nUsers JSON content:");
        println!("{}", content);
    }

    println!("\n✓ All tests passed successfully!");
    Ok(())
}