// Searchable encryption: deterministic HMAC tags for equality lookups on
// encrypted fields, plus the env-var key provider (B3.8).

use base64::Engine;
use nosql_orm::encryption::{
  token_field, EncryptedStorage, EnvKeyProvider, KeyProvider, StaticKeyProvider,
};
use nosql_orm::error::OrmError;
use nosql_orm::provider::Storage;
use nosql_orm::providers::JsonProvider;
use nosql_orm::query::Filter;
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

async fn encrypted_store(dir: &std::path::Path) -> EncryptedStorage<JsonProvider> {
  let inner = JsonProvider::new(dir).await.unwrap();
  let key = Arc::new(StaticKeyProvider::new(vec![7u8; 32]).unwrap()) as Arc<dyn KeyProvider>;
  EncryptedStorage::new(inner, key, vec!["email".to_string()])
    .with_searchable(vec!["email".to_string()])
}

#[tokio::test]
async fn equality_filter_finds_docs_by_plaintext_on_encrypted_field() {
  let dir = tempdir().unwrap();
  let store = encrypted_store(dir.path()).await;

  store
    .insert(
      "users",
      json!({"id": "1", "email": "alice@example.com", "team": "red"}),
    )
    .await
    .unwrap();
  store
    .insert(
      "users",
      json!({"id": "2", "email": "bob@example.com", "team": "blue"}),
    )
    .await
    .unwrap();

  let filter = Filter::Eq("email".to_string(), json!("alice@example.com"));
  let hits = store
    .find_many("users", Some(&filter), None, None, None, true)
    .await
    .unwrap();

  assert_eq!(hits.len(), 1);
  assert_eq!(hits[0]["id"], "1");
  // Readable plaintext, and no token detail leaks into the result.
  assert_eq!(hits[0]["email"], "alice@example.com");
  assert!(hits[0].get("email__token").is_none());

  // Non-matching plaintext finds nothing (the ciphertext never matches).
  let miss = Filter::Eq("email".to_string(), json!("nobody@example.com"));
  let hits = store
    .find_many("users", Some(&miss), None, None, None, true)
    .await
    .unwrap();
  assert!(hits.is_empty());
}

#[tokio::test]
async fn stored_value_is_ciphertext_and_token_is_deterministic() {
  let dir = tempdir().unwrap();
  let store = encrypted_store(dir.path()).await;

  store
    .insert("users", json!({"id": "1", "email": "alice@example.com"}))
    .await
    .unwrap();
  store
    .insert("users", json!({"id": "2", "email": "alice@example.com"}))
    .await
    .unwrap();

  let raw = JsonProvider::new(dir.path()).await.unwrap();
  let raw1 = raw.find_by_id("users", "1").await.unwrap().unwrap();
  let raw2 = raw.find_by_id("users", "2").await.unwrap().unwrap();

  // The value itself is encrypted...
  assert!(raw1["email"].as_str().unwrap().starts_with("enc:"));
  assert!(raw2["email"].as_str().unwrap().starts_with("enc:"));

  // ...and equal plaintexts share one deterministic tag: that is what an
  // equality index matches on.
  let token1 = raw1["email__token"].as_str().unwrap().to_string();
  let token2 = raw2["email__token"].as_str().unwrap().to_string();
  assert!(token1.starts_with("hmac:"));
  assert_eq!(token1, token2);

  let other = store
    .insert("users", json!({"id": "3", "email": "bob@example.com"}))
    .await
    .unwrap();
  assert!(
    other.get("email__token").is_none(),
    "read result carries no token"
  );
}

#[tokio::test]
async fn count_and_delete_many_rewrite_searchable_filters() {
  let dir = tempdir().unwrap();
  let store = encrypted_store(dir.path()).await;

  store
    .insert("users", json!({"id": "1", "email": "alice@example.com"}))
    .await
    .unwrap();
  store
    .insert("users", json!({"id": "2", "email": "alice@example.com"}))
    .await
    .unwrap();
  store
    .insert("users", json!({"id": "3", "email": "bob@example.com"}))
    .await
    .unwrap();

  let filter = Filter::Eq("email".to_string(), json!("alice@example.com"));
  assert_eq!(store.count("users", Some(&filter)).await.unwrap(), 2);

  let removed = store.delete_many("users", Some(filter)).await.unwrap();
  assert_eq!(removed, 2);
  assert_eq!(store.count("users", None).await.unwrap(), 1);
}

#[tokio::test]
async fn in_filter_on_searchable_field_matches_any_plaintext() {
  let dir = tempdir().unwrap();
  let store = encrypted_store(dir.path()).await;

  store
    .insert("users", json!({"id": "1", "email": "alice@example.com"}))
    .await
    .unwrap();
  store
    .insert("users", json!({"id": "2", "email": "bob@example.com"}))
    .await
    .unwrap();
  store
    .insert("users", json!({"id": "3", "email": "carol@example.com"}))
    .await
    .unwrap();

  let filter = Filter::In(
    "email".to_string(),
    vec![json!("alice@example.com"), json!("carol@example.com")],
  );
  let hits = store
    .find_many("users", Some(&filter), None, None, None, true)
    .await
    .unwrap();
  let mut ids: Vec<&str> = hits.iter().map(|h| h["id"].as_str().unwrap()).collect();
  ids.sort();
  assert_eq!(ids, vec!["1", "3"]);
}

#[tokio::test]
async fn nested_and_or_filters_are_rewritten_too() {
  let dir = tempdir().unwrap();
  let store = encrypted_store(dir.path()).await;

  store
    .insert("users", json!({"id": "1", "email": "alice@example.com"}))
    .await
    .unwrap();
  store
    .insert("users", json!({"id": "2", "email": "bob@example.com"}))
    .await
    .unwrap();

  let filter = Filter::Or(vec![
    Filter::And(vec![Filter::Eq(
      "email".to_string(),
      json!("alice@example.com"),
    )]),
    Filter::Eq("email".to_string(), json!("nobody@example.com")),
  ]);
  let hits = store
    .find_many("users", Some(&filter), None, None, None, true)
    .await
    .unwrap();
  assert_eq!(hits.len(), 1);
  assert_eq!(hits[0]["id"], "1");
}

#[tokio::test]
async fn searchable_index_is_declared_and_validated() {
  let dir = tempdir().unwrap();
  let store = encrypted_store(dir.path()).await;

  store
    .insert("users", json!({"id": "1", "email": "alice@example.com"}))
    .await
    .unwrap();

  // Indexing a searchable field succeeds (the JSON provider accepts index
  // declarations as no-ops); a field that is not searchable is refused.
  store
    .create_searchable_index("users", "email")
    .await
    .unwrap();
  let err = store
    .create_searchable_index("users", "team")
    .await
    .unwrap_err();
  assert!(matches!(err, OrmError::InvalidInput(_)));

  // The companion field name is part of the contract.
  assert_eq!(token_field("email"), "email__token");
}

#[test]
fn env_key_provider_reads_raw_and_base64_keys() {
  std::env::set_var("NOSQL_ORM_TEST_KEY_RAW", "0123456789abcdef0123456789abcdef");
  let raw = EnvKeyProvider::from_env("NOSQL_ORM_TEST_KEY_RAW").unwrap();
  assert_eq!(raw.key_bytes().len(), 32);

  let encoded = base64::engine::general_purpose::STANDARD.encode([3u8; 32]);
  std::env::set_var("NOSQL_ORM_TEST_KEY_B64", &encoded);
  let b64 = EnvKeyProvider::from_env("NOSQL_ORM_TEST_KEY_B64").unwrap();
  assert_eq!(b64.key_bytes(), &[3u8; 32]);

  let missing = match EnvKeyProvider::from_env("NOSQL_ORM_TEST_KEY_DEFINITELY_NOT_SET") {
    Err(e) => e,
    Ok(_) => panic!("expected an error for an unset variable"),
  };
  assert!(matches!(missing, OrmError::Encryption(_)));

  std::env::set_var("NOSQL_ORM_TEST_KEY_SHORT", "too short");
  let short = match EnvKeyProvider::from_env("NOSQL_ORM_TEST_KEY_SHORT") {
    Err(e) => e,
    Ok(_) => panic!("expected an error for a short key"),
  };
  assert!(matches!(short, OrmError::Encryption(_)));
}

#[tokio::test]
async fn env_key_provider_drives_encryption_end_to_end() {
  let dir = tempdir().unwrap();
  let key = Arc::new(
    EnvKeyProvider::from_env({
      std::env::set_var("NOSQL_ORM_TEST_KEY_E2E", "0123456789abcdef0123456789abcdef");
      "NOSQL_ORM_TEST_KEY_E2E"
    })
    .unwrap(),
  ) as Arc<dyn KeyProvider>;
  let inner = JsonProvider::new(dir.path()).await.unwrap();
  let store = EncryptedStorage::new(inner, key, vec!["secret".to_string()]);

  store
    .insert("items", json!({"id": "1", "secret": "s3cret"}))
    .await
    .unwrap();

  let found = store.find_by_id("items", "1").await.unwrap().unwrap();
  assert_eq!(found["secret"], "s3cret");

  let raw = JsonProvider::new(dir.path()).await.unwrap();
  let raw_doc = raw.find_by_id("items", "1").await.unwrap().unwrap();
  assert!(raw_doc["secret"].as_str().unwrap().starts_with("enc:"));
}
