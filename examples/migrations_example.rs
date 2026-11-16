use nosql_orm::prelude::*;
use nosql_orm::{JsonMigration, MigrationMeta, MigrationRunner, SqlMigration};

#[tokio::main]
async fn main() -> OrmResult<()> {
  println!("=== Migration System Example ===\n");

  let provider = JsonProvider::new("./data").await?;
  let mut runner = MigrationRunner::new(provider.clone());

  println!("1. Creating migrations...\n");

  // A JsonMigration script is a JSON array of operations, executed in
  // order. `down_script` states how to undo it.
  let seed_settings = JsonMigration::new(
    1,
    "seed_settings",
    serde_json::json!([
      {"op": "create_collection", "collection": "settings"},
      {"op": "insert", "collection": "settings",
       "doc": {"id": "app", "theme": "dark", "language": "en"}}
    ]),
    serde_json::json!([
      {"op": "delete", "collection": "settings", "id": "app"},
      {"op": "drop_collection", "collection": "settings"}
    ]),
  );

  // `patch` applies an RFC 6902 subset (add / remove / replace) to one
  // document — the "backfill a field" migration.
  let add_language_preference = JsonMigration::new(
    2,
    "add_language_preference",
    serde_json::json!([
      {"op": "patch", "collection": "settings", "id": "app",
       "patch": [{"op": "add", "path": "/locale", "value": "en-US"}]}
    ]),
    serde_json::json!([
      {"op": "patch", "collection": "settings", "id": "app",
       "patch": [{"op": "remove", "path": "/locale"}]}
    ]),
  );

  runner.add_migration(seed_settings);
  runner.add_migration(add_language_preference);

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

  // The bodies really ran — not just the bookkeeping.
  let count = Storage::count(&provider, "settings", None).await?;
  let doc = Storage::find_by_id(&provider, "settings", "app")
    .await?
    .unwrap_or(serde_json::Value::Null);
  println!("   settings now holds {} document(s): {}\n", count, doc);

  println!("3. Checking migration status...");
  let status: Vec<MigrationMeta> = runner.status().await?;
  println!("   Total applied: {} migration(s)\n", status.len());

  println!("4. Rolling back the last migration...");
  runner.rollback(1).await?;
  let after = Storage::find_by_id(&provider, "settings", "app")
    .await?
    .unwrap_or(serde_json::Value::Null);
  println!(
    "   locale removed: {}, status now {} migration(s)\n",
    !after.get("locale").is_some(),
    runner.status().await?.len()
  );

  // A rolled-back migration is no longer recorded as applied, so the next
  // run picks it up again.
  let reapplied = runner.run_all_pending().await?;
  println!("   re-ran {} migration(s)\n", reapplied.len());

  println!("\n5. SqlMigration (needs a provider with a query language):");
  let sql_migration = SqlMigration::new(
    3,
    "create_posts_table",
    "CREATE TABLE posts (id TEXT PRIMARY KEY, title TEXT NOT NULL);",
    "DROP TABLE posts;",
  );
  println!(
    "   v{}: {} — up: {:?}",
    sql_migration.version, sql_migration.name, sql_migration.up_sql
  );
  println!("   (run it against SqliteProvider/PostgresProvider; a JSON");
  println!("    provider answers NotSupported because it has no SQL.)");

  println!("\n=== Done ===");
  Ok(())
}
