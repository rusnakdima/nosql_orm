use nosql_orm::prelude::*;
use nosql_orm::{JsonMigration, Migration, MigrationMeta, MigrationRunner, SqlMigration};

#[tokio::main]
async fn main() -> OrmResult<()> {
  println!("=== Migration System Example ===\n");

  let provider = JsonProvider::new("./data").await?;
  let runner = MigrationRunner::new(provider.clone());

  println!("1. Creating migrations...\n");

  let sql_migration = SqlMigration::new(
    1,
    "create_users_table",
    "CREATE TABLE users (id TEXT PRIMARY KEY, name TEXT NOT NULL);",
    "DROP TABLE users;",
  );

  let json_migration = JsonMigration::new(
    2,
    "add_email_field",
    serde_json::json!([{"op": "add", "path": "/email", "value": ""}]),
    serde_json::json!([{"op": "remove", "path": "/email"}]),
  );

  let mut runner = runner;
  runner.add_migration(sql_migration);
  runner.add_migration(json_migration);

  println!("2. Running pending migrations...");
  let applied: Vec<MigrationMeta> = runner.run_all_pending().await?;
  println!("   Applied {} migration(s)\n", applied.len());

  for meta in &applied {
    println!(
      "   - v{}: {} (at {})",
      meta.version,
      meta.name,
      meta.applied_at.unwrap_or_default()
    );
  }

  println!("\n3. Checking migration status...");
  let status: Vec<MigrationMeta> = runner.status().await?;
  println!("   Total applied: {} migration(s)\n", status.len());

  println!("4. Migration trait demo - SqlMigration struct:");
  let migration = SqlMigration::new(
    3,
    "create_posts_table",
    "CREATE TABLE posts (id TEXT PRIMARY KEY, title TEXT NOT NULL);",
    "DROP TABLE posts;",
  );
  println!("   (SqlMigration stores up/down SQL for SQL databases)");

  println!("\n=== Done ===");
  Ok(())
}
