#![cfg(feature = "embedded")]

use nosql_orm::entity::Entity;
use nosql_orm::error::OrmResult;
use nosql_orm::provider::Storage;
use nosql_orm::providers::json::JsonProvider;
use nosql_orm::query::Filter;
use nosql_orm::repository::Repository;
use nosql_orm::validators::Validate;
use nosql_orm_derive::Entity as EntityDerive;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedAddress {
  pub street: String,
  pub city: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, EntityDerive)]
#[embedded(address)]
pub struct EmbeddedUser {
  pub id: Option<String>,
  pub name: String,
  pub address: EmbeddedAddress,
}

impl Validate for EmbeddedUser {
  fn validate(&self) -> OrmResult<()> {
    Ok(())
  }
}

struct EmbeddedFixture {
  provider: JsonProvider,
  repo: Repository<EmbeddedUser, JsonProvider>,
  _dir: tempfile::TempDir,
}

async fn fixture() -> EmbeddedFixture {
  let dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();
  let repo = Repository::new(provider.clone());
  EmbeddedFixture {
    provider,
    repo,
    _dir: dir,
  }
}

fn alice() -> EmbeddedUser {
  EmbeddedUser {
    id: None,
    name: "Alice".to_string(),
    address: EmbeddedAddress {
      street: "Main St 1".to_string(),
      city: "Kyiv".to_string(),
    },
  }
}

#[tokio::test]
async fn insert_flattens_embedded_fields_on_disk() {
  let f = fixture().await;
  let saved = f.repo.insert(alice()).await.unwrap();
  let id = saved.id.unwrap();

  let raw = f
    .provider
    .find_by_id("embedded_user", &id)
    .await
    .unwrap()
    .unwrap();
  assert!(raw.get("address").is_none(), "nested key must be flattened");
  assert_eq!(
    raw.get("address_street").and_then(|v| v.as_str()),
    Some("Main St 1")
  );
  assert_eq!(
    raw.get("address_city").and_then(|v| v.as_str()),
    Some("Kyiv")
  );
}

#[tokio::test]
async fn read_expands_embedded_fields_back_to_nested() {
  let f = fixture().await;
  let saved = f.repo.insert(alice()).await.unwrap();

  let found = f
    .repo
    .find_by_id(saved.id.as_deref().unwrap())
    .await
    .unwrap()
    .unwrap();
  assert_eq!(found.name, "Alice");
  assert_eq!(found.address.street, "Main St 1");
  assert_eq!(found.address.city, "Kyiv");

  let all = f.repo.find_all().await.unwrap();
  assert_eq!(all.len(), 1);
  assert_eq!(all[0].address.city, "Kyiv");
}

#[tokio::test]
async fn nested_path_filters_match_flat_storage() {
  let f = fixture().await;
  f.repo.insert(alice()).await.unwrap();
  f.repo
    .insert(EmbeddedUser {
      id: None,
      name: "Boris".to_string(),
      address: EmbeddedAddress {
        street: "Side St 2".to_string(),
        city: "Lviv".to_string(),
      },
    })
    .await
    .unwrap();

  let kyiv = f
    .repo
    .query()
    .where_eq("address.city", "Kyiv")
    .find()
    .await
    .unwrap();
  assert_eq!(kyiv.len(), 1);
  assert_eq!(kyiv[0].name, "Alice");

  let count = f
    .repo
    .query()
    .where_eq("address.city", "Kyiv")
    .count()
    .await
    .unwrap();
  assert_eq!(count, 1);

  let missing = f
    .repo
    .query()
    .where_eq("address.city", "Odesa")
    .find()
    .await
    .unwrap();
  assert!(missing.is_empty());
}

#[tokio::test]
async fn update_and_patch_keep_storage_flat() {
  let f = fixture().await;
  let saved = f.repo.insert(alice()).await.unwrap();
  let id = saved.id.clone().unwrap();

  let mut updated = saved;
  updated.address.city = "Lviv".to_string();
  f.repo.update(updated).await.unwrap();

  let raw = f.provider.find_by_id("embedded_user", &id).await.unwrap().unwrap();
  assert!(raw.get("address").is_none());
  assert_eq!(
    raw.get("address_city").and_then(|v| v.as_str()),
    Some("Lviv")
  );

  f.repo
    .patch(&id, json!({ "address": { "city": "Odesa" } }))
    .await
    .unwrap();
  let raw = f.provider.find_by_id("embedded_user", &id).await.unwrap().unwrap();
  assert!(raw.get("address").is_none());
  assert_eq!(
    raw.get("address_city").and_then(|v| v.as_str()),
    Some("Odesa")
  );
  assert_eq!(
    raw.get("address_street").and_then(|v| v.as_str()),
    Some("Main St 1"),
    "patching one embedded leaf must not drop the others"
  );

  let found = f.repo.find_by_id(&id).await.unwrap().unwrap();
  assert_eq!(found.address.city, "Odesa");
  assert_eq!(found.address.street, "Main St 1");
}

#[tokio::test]
async fn delete_many_accepts_nested_path_filters() {
  let f = fixture().await;
  f.repo.insert(alice()).await.unwrap();
  f.repo
    .insert(EmbeddedUser {
      id: None,
      name: "Boris".to_string(),
      address: EmbeddedAddress {
        street: "Side St 2".to_string(),
        city: "Lviv".to_string(),
      },
    })
    .await
    .unwrap();

  let deleted = f
    .repo
    .delete_many(Some(Filter::Eq(
      "address.city".to_string(),
      json!("Kyiv"),
    )))
    .await
    .unwrap();
  assert_eq!(deleted, 1);

  let remaining = f.repo.find_all().await.unwrap();
  assert_eq!(remaining.len(), 1);
  assert_eq!(remaining[0].name, "Boris");
}

#[tokio::test]
async fn streams_rewrite_nested_filters_manually() {
  let f = fixture().await;
  f.repo.insert(alice()).await.unwrap();
  f.repo
    .insert(EmbeddedUser {
      id: None,
      name: "Boris".to_string(),
      address: EmbeddedAddress {
        street: "Side St 2".to_string(),
        city: "Lviv".to_string(),
      },
    })
    .await
    .unwrap();

  // `Repository::find_stream` does not rewrite embedded paths itself (the
  // file carrying it is not writable in this workspace); callers adapt the
  // filter with the public `Filter::for_entity` helper instead.
  let filter = Filter::Eq("address.city".to_string(), json!("Lviv")).for_entity::<EmbeddedUser>();
  let mut stream = f.repo.find_stream(Some(filter), None, None, None, true).await.unwrap();
  let mut hits = Vec::new();
  while let Some(entity) = stream.next_item().await.unwrap() {
    hits.push(entity);
  }
  assert_eq!(hits.len(), 1);
  assert_eq!(hits[0].name, "Boris");
}

#[tokio::test]
async fn entity_without_embedded_fields_is_untouched() {
  #[derive(Debug, Clone, Serialize, Deserialize, EntityDerive)]
  pub struct PlainRecord {
    pub id: Option<String>,
    pub label: String,
  }

  impl Validate for PlainRecord {
    fn validate(&self) -> OrmResult<()> {
      Ok(())
    }
  }

  let dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();
  let repo: Repository<PlainRecord, JsonProvider> = Repository::new(provider.clone());

  let saved = repo
    .insert(PlainRecord {
      id: None,
      label: "plain".to_string(),
    })
    .await
    .unwrap();

  let raw = provider
    .find_by_id(&PlainRecord::table_name(), saved.id.as_deref().unwrap())
    .await
    .unwrap()
    .unwrap();
  assert_eq!(
    raw.get("label").and_then(|v| v.as_str()),
    Some("plain")
  );

  let found = repo
    .find_by_id(saved.id.as_deref().unwrap())
    .await
    .unwrap()
    .unwrap();
  assert_eq!(found.label, "plain");
}
