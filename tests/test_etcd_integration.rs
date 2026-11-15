// etcd integration tests.
//
// etcd is a server, so these are #[ignore]d like the Postgres/MySQL/Mongo
// suites. When they are run explicitly with a server reachable they exercise
// the real `Storage` contract; when the server is unreachable they skip
// instead of failing, so `cargo test --features etcd` stays green either way.

use nosql_orm::error::{OrmError, OrmResult};
use nosql_orm::provider::{
  AdminCommands, Capability, SchemaIntrospection, Storage, TransactionControl,
};
use nosql_orm::providers::etcd::EtcdProvider;
use nosql_orm::query::Filter;
use serde_json::json;

fn endpoint() -> String {
  std::env::var("ETCD_ENDPOINTS").unwrap_or_else(|_| "http://127.0.0.1:2379".to_string())
}

// Connect, or report the server as absent so the caller can skip.
async fn provider_or_skip() -> OrmResult<Option<EtcdProvider>> {
  match EtcdProvider::connect(&endpoint()).await {
    Ok(p) => Ok(Some(p)),
    Err(e) => {
      eprintln!(
        "skipping etcd test: cannot connect to {}: {}",
        endpoint(),
        e
      );
      Ok(None)
    }
  }
}

// Each test uses its own prefix so parallel runs never share keys, and the
// whole collection is dropped at the end.
async fn cleanup(provider: &EtcdProvider) {
  let _ = provider.drop_collection("docs").await;
}

#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_satisfies_the_storage_contract() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let provider = provider.with_prefix("nosql_orm_test/contract/");
  provider.drop_collection("docs").await?;

  // Insert honours a caller-supplied id and reads back verbatim.
  let ann = provider
    .insert("docs", json!({"id": "c-1", "name": "Ann", "score": 3}))
    .await?;
  assert_eq!(ann["id"], "c-1");
  provider
    .insert("docs", json!({"id": "c-2", "name": "Bob", "score": 1}))
    .await?;
  provider
    .insert("docs", json!({"id": "c-3", "name": "Cid", "score": 2}))
    .await?;
  assert_eq!(provider.count("docs", None).await?, 3);

  // Read back by id; an unknown id is None, never an error.
  let ann = provider
    .find_by_id("docs", "c-1")
    .await?
    .ok_or_else(|| OrmError::NotFound("docs/c-1".to_string()))?;
  assert_eq!(ann["name"], "Ann");
  assert!(provider.find_by_id("docs", "missing").await?.is_none());

  // Equality filter selects exactly the match.
  let anns = provider
    .find_many(
      "docs",
      Some(&Filter::Eq("name".to_string(), json!("Ann"))),
      None,
      None,
      None,
      true,
    )
    .await?;
  assert_eq!(anns.len(), 1);
  assert_eq!(anns[0]["id"], "c-1");

  // Sort both ways, then paginate.
  let ascending = provider
    .find_many("docs", None, Some(0), Some(10), Some("score"), true)
    .await?;
  assert_eq!(
    ascending
      .iter()
      .map(|d| d["id"].as_str().unwrap())
      .collect::<Vec<_>>(),
    vec!["c-2", "c-3", "c-1"]
  );
  let page = provider
    .find_many("docs", None, Some(1), Some(1), Some("score"), true)
    .await?;
  assert_eq!(page.len(), 1);
  assert_eq!(page[0]["id"], "c-3");

  // Filtered count.
  assert_eq!(
    provider
      .count("docs", Some(&Filter::Gt("score".to_string(), json!(1))))
      .await?,
    2
  );

  cleanup(&provider).await;
  Ok(())
}

// A duplicate id is rejected — this is what the compare-and-swap insert buys.
#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_rejects_duplicate_ids() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let provider = provider.with_prefix("nosql_orm_test/dup/");
  provider.drop_collection("docs").await?;

  provider
    .insert("docs", json!({"id": "atomic-1", "name": "First"}))
    .await?;

  let rejected = provider
    .insert("docs", json!({"id": "atomic-1", "name": "Second"}))
    .await;
  assert!(
    matches!(&rejected, Err(OrmError::Duplicate(_))),
    "expected Duplicate, got {:?}",
    rejected
  );

  // The rejected insert must not have overwritten the row or changed the count.
  assert_eq!(provider.count("docs", None).await?, 1);
  let survivor = provider
    .find_by_id("docs", "atomic-1")
    .await?
    .ok_or_else(|| OrmError::NotFound("docs/atomic-1".to_string()))?;
  assert_eq!(survivor["name"], "First");

  cleanup(&provider).await;
  Ok(())
}

// update requires an existing document: it does not silently insert.
#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_update_requires_an_existing_document() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let provider = provider.with_prefix("nosql_orm_test/update/");
  provider.drop_collection("docs").await?;

  provider
    .insert("docs", json!({"id": "u-1", "name": "Ann", "score": 3}))
    .await?;

  let updated = provider
    .update(
      "docs",
      "u-1",
      json!({"id": "u-1", "name": "Zed", "score": 5}),
    )
    .await?;
  assert_eq!(updated["name"], "Zed");
  let reloaded = provider
    .find_by_id("docs", "u-1")
    .await?
    .ok_or_else(|| OrmError::NotFound("docs/u-1".to_string()))?;
  assert_eq!(reloaded["score"], 5);

  // Patch merges one field and leaves the rest alone.
  let patched = provider
    .patch("docs", "u-1", json!({"tier": "gold"}))
    .await?;
  assert_eq!(patched["name"], "Zed");
  assert_eq!(patched["tier"], "gold");

  // Updating a missing document is an error, not an insert.
  let missing = provider
    .update("docs", "nope", json!({"id": "nope", "name": "X"}))
    .await;
  assert!(
    matches!(&missing, Err(OrmError::NotFound(_))),
    "expected NotFound, got {:?}",
    missing
  );
  assert_eq!(provider.count("docs", None).await?, 1);

  cleanup(&provider).await;
  Ok(())
}

#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_bulk_and_delete_report_honest_counts() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let provider = provider.with_prefix("nosql_orm_test/bulk/");
  provider.drop_collection("docs").await?;

  provider
    .insert("docs", json!({"id": "b-1", "name": "Ann", "score": 3}))
    .await?;
  provider
    .insert("docs", json!({"id": "b-2", "name": "Bob", "score": 1}))
    .await?;
  provider
    .insert("docs", json!({"id": "b-3", "name": "Cid", "score": 2}))
    .await?;

  // update_many touches only the matches and reports how many.
  let touched = provider
    .update_many(
      "docs",
      Some(Filter::Lt("score".to_string(), json!(3))),
      json!({"tier": "low"}),
    )
    .await?;
  assert_eq!(touched, 2);
  assert_eq!(
    provider
      .count("docs", Some(&Filter::Eq("tier".to_string(), json!("low"))))
      .await?,
    2
  );

  // delete_many removes only the matches.
  let removed = provider
    .delete_many("docs", Some(Filter::Eq("name".to_string(), json!("Cid"))))
    .await?;
  assert_eq!(removed, 1);
  assert_eq!(provider.count("docs", None).await?, 2);

  // delete reports true once, then false.
  assert!(provider.delete("docs", "b-2").await?);
  assert!(!provider.delete("docs", "b-2").await?);
  assert_eq!(provider.count("docs", None).await?, 1);

  cleanup(&provider).await;
  Ok(())
}

#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_introspection_lists_what_it_wrote() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let provider = provider.with_prefix("nosql_orm_test/intro/");
  provider.drop_collection("docs").await?;

  provider
    .insert("docs", json!({"id": "i-1", "name": "Ann", "score": 3}))
    .await?;

  let expected = provider.count("docs", None).await?;
  let collections = provider.list_collections().await?;
  let listed = collections
    .iter()
    .find(|m| m.name == "docs")
    .ok_or_else(|| OrmError::NotFound("docs is not listed".to_string()))?;
  assert_eq!(
    listed.document_count, expected,
    "list_collections must agree with count()"
  );

  // Field names come from the documents.
  let schema = provider.describe_collection("docs").await?;
  assert!(schema.fields.contains_key("name"));
  assert!(schema.fields.contains_key("score"));

  let stats = provider.get_collection_stats("docs").await?;
  assert_eq!(stats.document_count, expected);

  assert_eq!(provider.get_database_name().await?, "etcd");

  cleanup(&provider).await;
  Ok(())
}

// etcd has no index engine and no query language; both must say so rather
// than pretend.
#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_reports_unsupported_capabilities_honestly() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let provider = provider.with_prefix("nosql_orm_test/caps/");

  assert!(!provider.supports(Capability::Indexes));
  assert!(!provider.supports(Capability::Transactions));
  assert!(!provider.supports(Capability::Aggregation));
  assert!(!provider.supports(Capability::RawQueries));

  assert!(matches!(
    provider
      .create_index(
        "docs",
        &nosql_orm::nosql_index::NosqlIndex::single("name", 1)
      )
      .await,
    Err(OrmError::NotSupported(_))
  ));
  assert!(matches!(
    provider.execute_raw("SELECT 1", vec![]).await,
    Err(OrmError::NotSupported(_))
  ));
  assert!(matches!(
    provider.aggregate("docs", vec![]).await,
    Err(OrmError::NotSupported(_))
  ));

  cleanup(&provider).await;
  Ok(())
}

// Every transaction entry point fails with NotSupported and no id is handed
// out that cannot undo anything.
#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_has_no_transaction_surface() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let provider = provider.with_prefix("nosql_orm_test/tx/");

  assert!(!provider.supports(Capability::Transactions));
  let begun = provider.begin_transaction().await;
  assert!(
    matches!(&begun, Err(OrmError::NotSupported(_))),
    "expected NotSupported, got {:?}",
    begun
  );
  assert!(!provider.is_transaction_active().await?);

  cleanup(&provider).await;
  Ok(())
}

// A collection name becomes part of the key prefix, so a '/' must be refused
// before it can make the prefix ambiguous.
#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_rejects_ambiguous_collection_names() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let provider = provider.with_prefix("nosql_orm_test/names/");

  assert!(matches!(
    provider.insert("a/b", json!({"id": "1"})).await,
    Err(OrmError::InvalidInput(_))
  ));
  assert!(matches!(
    provider.count("", None).await,
    Err(OrmError::InvalidInput(_))
  ));

  Ok(())
}

// rename moves every document and leaves nothing behind under the old name.
#[ignore = "needs a running etcd — set ETCD_ENDPOINTS"]
#[tokio::test]
async fn etcd_rename_collection_moves_documents() -> OrmResult<()> {
  let Some(provider) = provider_or_skip().await? else {
    return Ok(());
  };
  let provider = provider.with_prefix("nosql_orm_test/rename/");
  provider.drop_collection("old").await?;
  provider.drop_collection("new").await?;

  provider
    .insert("old", json!({"id": "r-1", "name": "Ann"}))
    .await?;
  provider
    .insert("old", json!({"id": "r-2", "name": "Bob"}))
    .await?;

  provider.rename_collection("old", "new").await?;

  assert_eq!(provider.count("old", None).await?, 0);
  assert_eq!(provider.count("new", None).await?, 2);
  let moved = provider
    .find_by_id("new", "r-1")
    .await?
    .ok_or_else(|| OrmError::NotFound("new/r-1".to_string()))?;
  assert_eq!(moved["name"], "Ann");

  provider.drop_collection("new").await?;
  Ok(())
}
