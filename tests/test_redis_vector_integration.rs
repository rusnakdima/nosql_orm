// Redis vector integration tests.
//
// Redis runs locally, so unlike the cloud-only vector stores these tests are
// `#[ignore]`d only by convention — they exercise a real server when you ask
// for them:
//
//   cargo test --features redis-vector --test test_redis_vector_integration -- --include-ignored
//
// What matters here is the vector half, not the storage half (which is
// `RedisProvider`'s own suite): an embedding is stored with the document, the
// nearest documents come back first, `top_k` truncates, and documents without
// a usable embedding are skipped rather than poisoning the result.

use nosql_orm::error::OrmResult;
use nosql_orm::provider::{AdminCommands, SchemaIntrospection, Storage};
use nosql_orm::providers::redis_vector::RedisVectorProvider;
use nosql_orm::vector_search::DistanceMetric;
use serde_json::json;

const URL: &str = "redis://127.0.0.1:6379";

fn endpoint() -> String {
  std::env::var("REDIS_CONNECTION").unwrap_or_else(|_| URL.to_string())
}

// One prefix per test so parallel runs never share keys; the collection is
// dropped at the end so a rerun starts empty.
async fn fresh(test: &str) -> OrmResult<Option<RedisVectorProvider>> {
  let provider = match RedisVectorProvider::new(&endpoint()).await {
    Ok(p) => p,
    Err(e) => {
      eprintln!("skipping redis vector test: {}", e);
      return Ok(None);
    }
  };
  let provider = provider.with_prefix(&format!("rvct:{}:", test));
  let _ = provider.drop_collection("docs").await;
  Ok(Some(provider))
}

// Three points on a plane: `near` sits on the query vector, `far` opposite.
const NEAR: [f64; 3] = [1.0, 0.0, 0.0];
const FAR: [f64; 3] = [0.0, 1.0, 0.0];
const MIDDLE: [f64; 3] = [0.7, 0.7, 0.0];

#[ignore = "needs a running Redis — set REDIS_CONNECTION"]
#[tokio::test]
async fn redis_vector_stores_the_embedding_and_returns_nearest_first() -> OrmResult<()> {
  let Some(provider) = fresh("contract").await? else {
    return Ok(());
  };

  provider
    .upsert_with_embedding("docs", json!({"id": "near", "name": "Near"}), NEAR.to_vec())
    .await?;
  provider
    .upsert_with_embedding(
      "docs",
      json!({"id": "middle", "name": "Middle"}),
      MIDDLE.to_vec(),
    )
    .await?;
  provider
    .upsert_with_embedding("docs", json!({"id": "far", "name": "Far"}), FAR.to_vec())
    .await?;

  // The embedding is stored with the document and reads back intact.
  let stored = provider
    .find_by_id("docs", "near")
    .await?
    .expect("document must exist");
  assert_eq!(
    stored["_embedding"],
    json!([1.0, 0.0, 0.0]),
    "the embedding must round-trip"
  );
  assert_eq!(provider.count("docs", None).await?, 3);

  // Nearest first: near (dist 0) < middle < far (dist 1).
  let hits = provider.search_similar("docs", &NEAR, 3).await?;
  let order: Vec<&str> = hits
    .iter()
    .map(|(d, _)| d["id"].as_str().unwrap_or(""))
    .collect();
  assert_eq!(order, vec!["near", "middle", "far"]);
  assert!(
    hits[0].1 < hits[1].1 && hits[1].1 < hits[2].1,
    "distances must increase with rank, got {:?}",
    hits.iter().map(|h| h.1).collect::<Vec<_>>()
  );

  provider.drop_collection("docs").await?;
  Ok(())
}

#[ignore = "needs a running Redis — set REDIS_CONNECTION"]
#[tokio::test]
async fn redis_vector_top_k_truncates() -> OrmResult<()> {
  let Some(provider) = fresh("topk").await? else {
    return Ok(());
  };

  for (id, emb) in [("a", NEAR), ("b", MIDDLE), ("c", FAR)] {
    provider
      .upsert_with_embedding("docs", json!({"id": id}), emb.to_vec())
      .await?;
  }

  let one = provider.search_similar("docs", &NEAR, 1).await?;
  assert_eq!(one.len(), 1, "top_k=1 must return exactly one");
  assert_eq!(one[0].0["id"], "a");

  let two = provider.search_similar("docs", &NEAR, 2).await?;
  assert_eq!(two.len(), 2);

  // Asking for more than exists returns everything, not an error.
  let all = provider.search_similar("docs", &NEAR, 99).await?;
  assert_eq!(all.len(), 3);

  provider.drop_collection("docs").await?;
  Ok(())
}

// Each metric must rank the identical vector as an exact match first; the
// three disagree only on how they score the rest.
#[ignore = "needs a running Redis — set REDIS_CONNECTION"]
#[tokio::test]
async fn redis_vector_supports_every_metric() -> OrmResult<()> {
  let Some(provider) = fresh("metrics").await? else {
    return Ok(());
  };

  for (id, emb) in [("match", NEAR), ("other", FAR)] {
    provider
      .upsert_with_embedding("docs", json!({"id": id}), emb.to_vec())
      .await?;
  }

  for metric in [
    DistanceMetric::Cosine,
    DistanceMetric::Euclidean,
    DistanceMetric::DotProduct,
  ] {
    let provider = provider.clone().with_metric(metric);
    let hits = provider.search_similar("docs", &NEAR, 2).await?;
    assert_eq!(
      hits[0].0["id"], "match",
      "metric {:?} must rank the identical vector first",
      metric
    );
    assert!(
      hits[0].1 < hits[1].1,
      "metric {:?}: the exact match must be strictly closest",
      metric
    );
  }

  provider.drop_collection("docs").await?;
  Ok(())
}

// A document with no embedding, or one of the wrong length, must be skipped
// rather than making the whole search fail.
#[ignore = "needs a running Redis — set REDIS_CONNECTION"]
#[tokio::test]
async fn redis_vector_skips_documents_without_a_usable_embedding() -> OrmResult<()> {
  let Some(provider) = fresh("skip").await? else {
    return Ok(());
  };

  provider
    .upsert_with_embedding("docs", json!({"id": "good"}), NEAR.to_vec())
    .await?;
  // No embedding at all.
  provider
    .insert("docs", json!({"id": "bare", "name": "no vector"}))
    .await?;
  // Wrong dimensionality for a 3-d query.
  provider
    .upsert_with_embedding("docs", json!({"id": "short"}), vec![1.0, 0.0])
    .await?;

  let hits = provider.search_similar("docs", &NEAR, 10).await?;
  assert_eq!(hits.len(), 1, "only the well-formed vector should match");
  assert_eq!(hits[0].0["id"], "good");

  provider.drop_collection("docs").await?;
  Ok(())
}

// The embedding field is configurable, so a caller can keep their own name.
#[ignore = "needs a running Redis — set REDIS_CONNECTION"]
#[tokio::test]
async fn redis_vector_honours_a_custom_embedding_field() -> OrmResult<()> {
  let Some(provider) = fresh("field").await? else {
    return Ok(());
  };

  let provider = provider.with_embedding_field("vec");
  provider
    .upsert_with_embedding("docs", json!({"id": "x"}), NEAR.to_vec())
    .await?;

  let stored = provider
    .find_by_id("docs", "x")
    .await?
    .expect("document must exist");
  assert!(stored.get("vec").is_some(), "custom field must be used");
  assert!(
    stored.get("_embedding").is_none(),
    "the default field must not be written when overridden"
  );

  let hits = provider.search_similar("docs", &NEAR, 1).await?;
  assert_eq!(hits.len(), 1);

  provider.drop_collection("docs").await?;
  Ok(())
}

// It is still a Redis provider: ordinary storage and introspection work, and
// Redis's honest `NotSupported` answers are inherited unchanged.
#[ignore = "needs a running Redis — set REDIS_CONNECTION"]
#[tokio::test]
async fn redis_vector_keeps_the_redis_storage_contract() -> OrmResult<()> {
  let Some(provider) = fresh("contract2").await? else {
    return Ok(());
  };

  provider
    .insert("docs", json!({"id": "c-1", "name": "Ann", "score": 3}))
    .await?;
  provider
    .insert("docs", json!({"id": "c-2", "name": "Bob", "score": 1}))
    .await?;
  assert_eq!(provider.count("docs", None).await?, 2);

  let collections = provider.list_collections().await?;
  assert!(
    collections.iter().any(|m| m.name == "docs"),
    "the collection must be listed, got {:?}",
    collections
  );

  assert_eq!(provider.get_database_name().await?, "redis");
  assert!(provider.health_check().await?);

  provider.drop_collection("docs").await?;
  Ok(())
}
