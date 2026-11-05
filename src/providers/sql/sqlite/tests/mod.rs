use super::*;
use tempfile::tempdir;

async fn make_table(provider: &SqliteProvider, columns: &[(&str, &str)]) -> OrmResult<()> {
  let columns = columns
    .iter()
    .map(|(name, kind)| format!("{} {}", name, kind))
    .collect::<Vec<_>>()
    .join(", ");
  let sql = format!("CREATE TABLE users ({})", columns);
  provider.execute_raw(&sql, Vec::new()).await?;
  Ok(())
}

#[tokio::test]
async fn sqlite_insert_find_by_id() -> OrmResult<()> {
  let temp = tempdir()?;
  let db_path = temp.path().join("test.db");
  let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
  make_table(
    &provider,
    &[("id", "TEXT"), ("name", "TEXT"), ("email", "TEXT")],
  )
  .await?;

  let doc = serde_json::json!({
    "id": "test-id-1",
    "name": "Alice",
    "email": "alice@example.com"
  });
  let inserted = provider.insert("users", doc.clone()).await?;
  assert_eq!(inserted["id"], "test-id-1");
  assert_eq!(inserted["name"], "Alice");

  let found = provider.find_by_id("users", "test-id-1").await?;
  assert!(found.is_some());
  assert_eq!(found.unwrap()["name"], "Alice");

  Ok(())
}

#[tokio::test]
async fn sqlite_insert_delete() -> OrmResult<()> {
  let temp = tempdir()?;
  let db_path = temp.path().join("test.db");
  let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
  make_table(&provider, &[("id", "TEXT"), ("name", "TEXT")]).await?;

  let doc = serde_json::json!({"id": "delete-me", "name": "Bob"});
  provider.insert("users", doc).await?;

  let found_before = provider.find_by_id("users", "delete-me").await?;
  assert!(found_before.is_some());

  provider.delete("users", "delete-me").await?;

  let found_after = provider.find_by_id("users", "delete-me").await?;
  assert!(found_after.is_none());

  Ok(())
}

#[tokio::test]
async fn sqlite_update() -> OrmResult<()> {
  let temp = tempdir()?;
  let db_path = temp.path().join("test.db");
  let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
  make_table(
    &provider,
    &[("id", "TEXT"), ("name", "TEXT"), ("age", "INTEGER")],
  )
  .await?;

  let doc = serde_json::json!({"id": "update-me", "name": "Charlie", "age": 30});
  provider.insert("users", doc).await?;

  let updated = provider
    .update(
      "users",
      "update-me",
      serde_json::json!({"name": "Charles", "age": 31}),
    )
    .await?;
  assert_eq!(updated["name"], "Charles");
  assert_eq!(updated["age"], 31);

  let found = provider.find_by_id("users", "update-me").await?;
  assert!(found.is_some());
  assert_eq!(found.unwrap()["name"], "Charles");

  Ok(())
}

#[tokio::test]
async fn sqlite_find_many_pages_and_sorts() -> OrmResult<()> {
  let temp = tempdir()?;
  let db_path = temp.path().join("test.db");
  let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
  make_table(&provider, &[("id", "TEXT"), ("name", "TEXT")]).await?;

  for (id, name) in [("a", "Ann"), ("b", "Bob"), ("c", "Cid")] {
    provider
      .insert("users", serde_json::json!({"id": id, "name": name}))
      .await?;
  }

  let all = provider
    .find_many("users", None, Some(0), Some(10), None, true)
    .await?;
  assert_eq!(all.len(), 3);

  let page = provider
    .find_many("users", None, Some(0), Some(2), None, true)
    .await?;
  assert_eq!(page.len(), 2);

  let sorted = provider
    .find_many("users", None, Some(0), Some(10), Some("name"), false)
    .await?;
  assert_eq!(sorted[0]["name"], "Cid");

  let counted = provider.count("users", None).await?;
  assert_eq!(counted, 3);

  Ok(())
}

#[tokio::test]
async fn sqlite_list_collections_counts_the_rows() -> OrmResult<()> {
  let temp = tempdir()?;
  let db_path = temp.path().join("test.db");
  let provider = SqliteProvider::connect(db_path.to_str().unwrap()).await?;
  make_table(&provider, &[("id", "TEXT"), ("name", "TEXT")]).await?;
  provider
    .insert("users", serde_json::json!({"id": "u1", "name": "Ann"}))
    .await?;

  let collections = provider.list_collections().await?;
  let users = collections
    .iter()
    .find(|c| c.name == "users")
    .expect("the users table is listed");
  assert_eq!(users.document_count, 1);

  Ok(())
}
