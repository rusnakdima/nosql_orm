// InfluxDB integration tests.
//
// InfluxDB is a server, so these are #[ignore]d like the Postgres/MySQL/Mongo
// suites. Each test uses its own database because the suites run in parallel
// and a shared measurement would let one test's points be another's.
//
// Run them with:
//   cargo test --features influxdb --test test_influxdb_integration -- --include-ignored
//
// The suite leans on the one real mismatch between a time-series store and a
// document store: writing an existing id APPENDS a second point, so every read
// and every count has to dedupe with `LAST(...) GROUP BY id`. `count` is
// asserted after an update specifically, because that is where a naive
// point-count would drift away from `find_many`.

use nosql_orm::error::{OrmError, OrmResult};
use nosql_orm::provider::{
  AdminCommands, Capability, SchemaIntrospection, Storage, TransactionControl,
};
use nosql_orm::providers::influxdb::InfluxDbProvider;
use nosql_orm::query::Filter;
use serde_json::json;

const URL: &str = "http://127.0.0.1:8086";

fn endpoint() -> String {
  std::env::var("INFLUXDB_URL").unwrap_or_else(|_| URL.to_string())
}

// One database per test: parallel runs must not share measurements.
//
// Returns `None` when the server cannot be reached, so the caller skips
// instead of failing — the convention the Postgres/MySQL suites use.
async fn fresh(test: &str) -> OrmResult<Option<InfluxDbProvider>> {
  let provider = match InfluxDbProvider::connect(&endpoint(), test).await {
    Ok(p) => p,
    Err(e) => {
      eprintln!("skipping InfluxDB test: {}", e);
      return Ok(None);
    }
  };
  // A rerun of the same test starts empty; a missing measurement is a no-op.
  let _ = provider.drop_collection("docs").await;
  Ok(Some(provider))
}

#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_satisfies_the_storage_contract() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_contract").await? else {
    return Ok(());
  };

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

  let ann = provider
    .find_by_id("docs", "c-1")
    .await?
    .ok_or_else(|| OrmError::NotFound("docs/c-1".to_string()))?;
  assert_eq!(ann["name"], "Ann");
  assert_eq!(ann["score"], 3);
  assert!(provider.find_by_id("docs", "missing").await?.is_none());

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

  let ascending = provider
    .find_many("docs", None, Some(0), Some(10), Some("score"), true)
    .await?;
  assert_eq!(ids(&ascending), vec!["c-2", "c-3", "c-1"]);
  let descending = provider
    .find_many("docs", None, Some(0), Some(10), Some("score"), false)
    .await?;
  assert_eq!(ids(&descending), vec!["c-1", "c-3", "c-2"]);
  let page = provider
    .find_many("docs", None, Some(1), Some(1), Some("score"), true)
    .await?;
  assert_eq!(ids(&page), vec!["c-3"]);

  assert_eq!(
    provider
      .count("docs", Some(&Filter::Gt("score".to_string(), json!(1))))
      .await?,
    2
  );

  let updated = provider
    .update(
      "docs",
      "c-1",
      json!({"id": "c-1", "name": "Zed", "score": 5}),
    )
    .await?;
  assert_eq!(updated["name"], "Zed");
  let reloaded = provider
    .find_by_id("docs", "c-1")
    .await?
    .ok_or_else(|| OrmError::NotFound("docs/c-1".to_string()))?;
  assert_eq!(reloaded["name"], "Zed");
  assert_eq!(reloaded["score"], 5);

  let patched = provider.patch("docs", "c-1", json!({"tier": "gold"})).await?;
  assert_eq!(patched["name"], "Zed");
  assert_eq!(patched["score"], 5);
  assert_eq!(patched["tier"], "gold");

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

  let removed = provider
    .delete_many("docs", Some(Filter::Eq("name".to_string(), json!("Cid"))))
    .await?;
  assert_eq!(removed, 1);
  assert_eq!(provider.count("docs", None).await?, 2);

  assert!(provider.delete("docs", "c-2").await?);
  assert!(!provider.delete("docs", "c-2").await?);
  assert_eq!(provider.count("docs", None).await?, 1);

  let remaining = provider.find_all("docs").await?;
  assert_eq!(ids(&remaining), vec!["c-1"]);
  assert_eq!(remaining[0]["name"], "Zed");

  provider.drop_collection("docs").await?;
  Ok(())
}

// The time-series quirk: a second write to an existing id appends a point.
// After an update the count must still be 1, not 2 — otherwise `count` and
// `find_many` disagree and the document store illusion breaks.
#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_count_and_read_dedupe_after_an_update() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_dedupe").await? else {
    return Ok(());
  };

  provider
    .insert("docs", json!({"id": "d-1", "name": "Ann", "score": 1}))
    .await?;
  assert_eq!(provider.count("docs", None).await?, 1);

  // Three updates = three extra points under the same id tag.
  for score in [2, 3, 4] {
    provider
      .update(
        "docs",
        "d-1",
        json!({"id": "d-1", "name": "Ann", "score": score}),
      )
      .await?;
  }

  // count must agree with find_many: one document, newest values.
  assert_eq!(
    provider.count("docs", None).await?,
    1,
    "count must dedupe points, not count them"
  );
  let all = provider.find_all("docs").await?;
  assert_eq!(all.len(), 1, "find_all must return one document");
  assert_eq!(all[0]["score"], 4, "the newest point must win");

  // The unfiltered count and the filtered count must agree with each other.
  let unfiltered = provider.count("docs", None).await?;
  let filtered = provider
    .count("docs", Some(&Filter::Eq("id".to_string(), json!("d-1"))))
    .await?;
  assert_eq!(unfiltered, filtered);

  provider.drop_collection("docs").await?;
  Ok(())
}

// A duplicate id is rejected, and the rejected write changes nothing.
#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_rejects_duplicate_ids() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_dup").await? else {
    return Ok(());
  };

  provider
    .insert(
      "docs",
      json!({"id": "atomic-1", "name": "First", "score": 1}),
    )
    .await?;

  let rejected = provider
    .insert(
      "docs",
      json!({"id": "atomic-1", "name": "Second", "score": 2}),
    )
    .await;
  assert!(
    matches!(&rejected, Err(OrmError::Duplicate(_))),
    "expected Duplicate, got {:?}",
    rejected
  );

  assert_eq!(provider.count("docs", None).await?, 1);
  let survivor = provider
    .find_by_id("docs", "atomic-1")
    .await?
    .ok_or_else(|| OrmError::NotFound("docs/atomic-1".to_string()))?;
  assert_eq!(
    survivor["name"], "First",
    "the rejected insert must not have overwritten the row"
  );

  provider.drop_collection("docs").await?;
  Ok(())
}

// update requires an existing document; it never silently inserts.
#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_update_requires_an_existing_document() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_update").await? else {
    return Ok(());
  };

  let missing = provider
    .update("docs", "nope", json!({"id": "nope", "name": "X"}))
    .await;
  assert!(
    matches!(&missing, Err(OrmError::NotFound(_))),
    "expected NotFound, got {:?}",
    missing
  );
  assert_eq!(provider.count("docs", None).await?, 0);

  provider
    .insert("docs", json!({"id": "u-1", "name": "Ann"}))
    .await?;
  let missing = provider
    .update("docs", "nope", json!({"id": "nope", "name": "X"}))
    .await;
  assert!(matches!(&missing, Err(OrmError::NotFound(_))));
  assert_eq!(provider.count("docs", None).await?, 1);

  provider.drop_collection("docs").await?;
  Ok(())
}

#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_introspection_lists_what_it_wrote() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_intro").await? else {
    return Ok(());
  };

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

  let schema = provider.describe_collection("docs").await?;
  assert!(schema.fields.contains_key("name"));
  assert!(schema.fields.contains_key("score"));

  let stats = provider.get_collection_stats("docs").await?;
  assert_eq!(stats.document_count, expected);

  assert_eq!(provider.get_database_name().await?, "nosql_orm_intro");
  let databases = provider.list_databases().await?;
  assert!(
    databases.iter().any(|d| d == "nosql_orm_intro"),
    "the test database must be listed, got {:?}",
    databases
  );

  provider.drop_collection("docs").await?;
  Ok(())
}

// `execute_raw` really runs InfluxQL — RawQueries is advertised because it
// works, not because it might.
#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_execute_raw_runs_influxql() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_raw").await? else {
    return Ok(());
  };

  assert!(provider.supports(Capability::RawQueries));

  provider
    .insert("docs", json!({"id": "r-1", "name": "Ann", "score": 3}))
    .await?;
  provider
    .insert("docs", json!({"id": "r-2", "name": "Bob", "score": 1}))
    .await?;

  let result = provider
    .execute_raw(r#"SELECT LAST(doc) FROM "docs" GROUP BY id"#, vec![])
    .await?;
  assert_eq!(result.columns.first().map(String::as_str), Some("time"));
  assert_eq!(
    result.affected_rows, 2,
    "one row per distinct id — GROUP BY emits one series per tag set"
  );

  // A DDL statement selects nothing and must not error.
  let ddl = provider.execute_raw(r#"SHOW MEASUREMENTS"#, vec![]).await?;
  assert!(!ddl.columns.is_empty());

  provider.drop_collection("docs").await?;
  Ok(())
}

// Indexes and aggregation have no InfluxDB equivalent behind our API, and
// transactions have no session. All three must say so.
#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_reports_unsupported_capabilities_honestly() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_caps").await? else {
    return Ok(());
  };

  assert!(!provider.supports(Capability::Indexes));
  assert!(!provider.supports(Capability::Transactions));
  assert!(!provider.supports(Capability::Aggregation));
  assert!(provider.supports(Capability::RawQueries));

  assert!(matches!(
    provider
      .create_index("docs", &nosql_orm::nosql_index::NosqlIndex::single("name", 1))
      .await,
    Err(OrmError::NotSupported(_))
  ));
  assert!(matches!(
    provider.aggregate("docs", vec![]).await,
    Err(OrmError::NotSupported(_))
  ));

  assert!(!provider.supports(Capability::Transactions));
  let begun = provider.begin_transaction().await;
  assert!(
    matches!(&begun, Err(OrmError::NotSupported(_))),
    "expected NotSupported, got {:?}",
    begun
  );
  assert!(!provider.is_transaction_active().await?);

  provider.drop_collection("docs").await?;
  Ok(())
}

// Measurement names are quoted into every query, so a name carrying InfluxQL
// metacharacters must still round-trip instead of corrupting the statement.
#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_quotes_measurement_names_with_metacharacters() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_quote").await? else {
    return Ok(());
  };

  let awkward = "my-meas.1";
  provider
    .insert(awkward, json!({"id": "q-1", "name": "Ann"}))
    .await?;
  assert_eq!(provider.count(awkward, None).await?, 1);
  assert!(provider.find_by_id(awkward, "q-1").await?.is_some());

  provider.drop_collection(awkward).await?;
  Ok(())
}

// An id containing a quote must not break out of the InfluxQL string literal.
#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_escapes_quotes_in_ids() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_escape").await? else {
    return Ok(());
  };

  let tricky = "it's a \"trap\"";
  provider
    .insert("docs", json!({"id": tricky, "name": "Ann"}))
    .await?;

  let found = provider.find_by_id("docs", tricky).await?;
  assert!(
    found.is_some(),
    "an id containing quotes must round-trip, got {:?}",
    found
  );
  assert_eq!(provider.count("docs", None).await?, 1);
  assert!(provider.delete("docs", tricky).await?);

  provider.drop_collection("docs").await?;
  Ok(())
}

#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_rename_collection_moves_documents() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_rename").await? else {
    return Ok(());
  };
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

#[ignore = "needs a running InfluxDB — set INFLUXDB_URL"]
#[tokio::test]
async fn influxdb_rejects_empty_collection_names() -> OrmResult<()> {
  let Some(provider) = fresh("nosql_orm_names").await? else {
    return Ok(());
  };

  assert!(matches!(
    provider.insert("", json!({"id": "1"})).await,
    Err(OrmError::InvalidInput(_))
  ));
  assert!(matches!(
    provider.count("", None).await,
    Err(OrmError::InvalidInput(_))
  ));

  Ok(())
}

fn ids(docs: &[serde_json::Value]) -> Vec<String> {
  docs
    .iter()
    .map(|d| d["id"].as_str().map(str::to_string).unwrap_or_default())
    .collect()
}
