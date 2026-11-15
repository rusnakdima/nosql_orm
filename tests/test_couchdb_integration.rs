use nosql_orm::error::OrmResult;
use nosql_orm::provider::{AdminCommands, Storage};
use nosql_orm::providers::couchdb::CouchDbProvider;
use serde_json::json;

fn endpoint() -> String {
  std::env::var("COUCHDB_URL").unwrap_or_else(|_| "http://127.0.0.1:5984".to_string())
}

async fn provider_or_skip() -> OrmResult<Option<CouchDbProvider>> {
  match CouchDbProvider::connect(&endpoint()).await {
    Ok(p) => Ok(Some(p)),
    Err(e) => {
      eprintln!("skipping couchdb test: {}", e);
      Ok(None)
    }
  }
}

#[ignore = "needs a running CouchDB"]
#[tokio::test]
async fn couchdb_basic_crud() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let _ = provider.drop_collection("docs").await;
  let ann = provider
    .insert("docs", json!({"id": "c-1", "name": "Ann"}))
    .await?;
  assert_eq!(ann["id"], "c-1");
  assert!(provider.find_by_id("docs", "c-1").await?.is_some());
  assert_eq!(provider.count("docs", None).await?, 1);
  let _ = provider.delete("docs", "c-1").await?;
  let _ = provider.drop_collection("docs").await;
  Ok(())
}
