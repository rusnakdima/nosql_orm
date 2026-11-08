// Roadmap D6 — provider conformance + per-backend transaction rollback.
//
// The remediation Phase 1 exit that was never run: every provider is held to
// the same `Storage` contract, the same contract is replayed through the
// `Arc<dyn StorageBackend>` bridge that `connect_backend` hands out, and
// every provider's answer to a transaction that fails mid-way is asserted.
//
// Two answers exist and both are asserted:
//
//   * a provider that advertises `Capability::Transactions` (SQLite — one
//     connection for the provider's lifetime) must roll a transaction that
//     failed mid-way back to exactly the state it had before
//     `begin_transaction`, and a commit must make its writes durable;
//   * a provider that does not (JSON, PostgreSQL, MySQL, MongoDB, Redis)
//     must fail every transaction entry point with `NotSupported` instead of
//     handing out an id that cannot undo anything, and a single write that
//     fails must leave no partial row behind.
//
// Hermetic: the JSON suite runs on the default feature set and the SQLite
// suite with `sql-sqlite`; both always run. The server-backed suites are
// `#[ignore]`d with the reason in the attribute — the convention
// tests/test_postgres_integration.rs already uses — and skip when the server
// is unreachable, so `cargo test --features sql` stays green without a
// PostgreSQL or MySQL instance.

use nosql_orm::admin_types::TransactionId;
use nosql_orm::error::{OrmError, OrmResult};
// `StorageBackend` is deliberately not imported: naming it in `use` would
// put it in method-resolution scope, and every provider here implements it
// through the blanket impl, so each `provider.count(...)` would become an
// ambiguity between the two traits. The dyn bridge below names it by path.
use nosql_orm::provider::{Capability, SchemaIntrospection, Storage, TransactionControl};
use nosql_orm::query::Filter;
use serde_json::{json, Value};

// The object `connect_backend` hands out; it implements `Storage`, not
// `SchemaIntrospection`, so the contract replays through it unchanged.
type DynBackend = std::sync::Arc<dyn nosql_orm::provider::StorageBackend>;

// The contract, replayed against any `Storage` — a concrete provider or the
// `Arc<dyn StorageBackend>` bridge.
//
// Only operations whose semantics are identical on every backend are
// asserted here; a backend that differs on purpose (Redis enforces no ids,
// MongoDB's replace does not reject an unknown id) states its difference in
// its own module rather than weakening this helper.
async fn assert_storage_contract<P>(provider: &P, collection: &str) -> OrmResult<()>
where
  P: Storage,
{
  // Insert: the caller's id is honoured and read back verbatim.
  let ann = provider
    .insert(collection, json!({"id": "c-1", "name": "Ann", "score": 3}))
    .await?;
  assert_eq!(ann["id"], "c-1");
  provider
    .insert(collection, json!({"id": "c-2", "name": "Bob", "score": 1}))
    .await?;
  provider
    .insert(collection, json!({"id": "c-3", "name": "Cid", "score": 2}))
    .await?;
  assert_eq!(provider.count(collection, None).await?, 3);

  // Read back by id; an unknown id is None — never an error, never a
  // phantom row.
  let ann = provider
    .find_by_id(collection, "c-1")
    .await?
    .ok_or_else(|| OrmError::NotFound(format!("{}/c-1", collection)))?;
  assert_eq!(ann["name"], "Ann");
  assert_eq!(ann["score"], 3);
  assert!(provider.exists(collection, "c-1").await?);
  assert!(!provider.exists(collection, "missing").await?);
  assert!(provider.find_by_id(collection, "missing").await?.is_none());

  // An equality filter selects exactly the matching document.
  let anns = provider
    .find_many(
      collection,
      Some(&Filter::Eq("name".to_string(), json!("Ann"))),
      None,
      None,
      None,
      true,
    )
    .await?;
  assert_eq!(ids(&anns)?, vec!["c-1"]);

  // Sort both ways over the sorted order, then paginate it.
  let ascending = provider
    .find_many(collection, None, Some(0), Some(10), Some("score"), true)
    .await?;
  assert_eq!(ids(&ascending)?, vec!["c-2", "c-3", "c-1"]);
  let descending = provider
    .find_many(collection, None, Some(0), Some(10), Some("score"), false)
    .await?;
  assert_eq!(ids(&descending)?, vec!["c-1", "c-3", "c-2"]);
  let page = provider
    .find_many(collection, None, Some(1), Some(1), Some("score"), true)
    .await?;
  assert_eq!(ids(&page)?, vec!["c-3"]);

  // A filtered count.
  assert_eq!(
    provider
      .count(collection, Some(&Filter::Gt("score".to_string(), json!(1))))
      .await?,
    2
  );

  // Update replaces the document and is read back with the new values.
  let updated = provider
    .update(
      collection,
      "c-1",
      json!({"id": "c-1", "name": "Zed", "score": 5}),
    )
    .await?;
  assert_eq!(updated["name"], "Zed");
  assert_eq!(updated["score"], 5);
  let reloaded = provider
    .find_by_id(collection, "c-1")
    .await?
    .ok_or_else(|| OrmError::NotFound(format!("{}/c-1 after update", collection)))?;
  assert_eq!(reloaded["name"], "Zed");
  assert_eq!(reloaded["score"], 5);

  // Patch merges one field and leaves the rest of the document alone.
  let patched = provider
    .patch(collection, "c-1", json!({"tier": "gold"}))
    .await?;
  assert_eq!(patched["name"], "Zed");
  assert_eq!(patched["score"], 5);
  assert_eq!(patched["tier"], "gold");

  // update_many applies to every match and reports how many it touched.
  let touched = provider
    .update_many(
      collection,
      Some(Filter::Lt("score".to_string(), json!(3))),
      json!({"tier": "low"}),
    )
    .await?;
  assert_eq!(touched, 2);
  assert_eq!(
    provider
      .count(
        collection,
        Some(&Filter::Eq("tier".to_string(), json!("low")))
      )
      .await?,
    2
  );

  // delete_many removes every match and reports the removals.
  let removed = provider
    .delete_many(
      collection,
      Some(Filter::Eq("name".to_string(), json!("Cid"))),
    )
    .await?;
  assert_eq!(removed, 1);
  assert_eq!(provider.count(collection, None).await?, 2);

  // delete reports what it removed: true once, false for the repeat.
  assert!(provider.delete(collection, "c-2").await?);
  assert!(!provider.delete(collection, "c-2").await?);
  assert_eq!(provider.count(collection, None).await?, 1);

  // find_all is the unfiltered read the Repository falls back to.
  let remaining = provider.find_all(collection).await?;
  assert_eq!(ids(&remaining)?, vec!["c-1"]);
  assert_eq!(remaining[0]["name"], "Zed");

  Ok(())
}

// Introspection: the provider lists the collection it just wrote, with the
// same row count the Storage side reports.
async fn assert_collection_is_listed<P>(provider: &P, collection: &str) -> OrmResult<()>
where
  P: Storage + SchemaIntrospection,
{
  let expected = provider.count(collection, None).await?;
  let collections = provider.list_collections().await?;
  let listed = collections
    .iter()
    .find(|meta| meta.name == collection)
    .ok_or_else(|| OrmError::NotFound(format!("collection {} is not listed", collection)))?;
  assert_eq!(
    listed.document_count, expected,
    "list_collections must report the row count count() reports"
  );
  Ok(())
}

// A write that fails leaves nothing behind: no new row, no changed row.
// A duplicate id is rejected by every store that enforces ids; Redis does
// not enforce them (last-write-wins by design) and says so in its own
// module instead of claiming this property.
async fn assert_failed_write_leaves_no_partial_writes<P>(
  provider: &P,
  collection: &str,
) -> OrmResult<()>
where
  P: Storage,
{
  provider
    .insert(
      collection,
      json!({"id": "atomic-1", "name": "First", "score": 1}),
    )
    .await?;
  let before = provider.count(collection, None).await?;

  let rejected = provider
    .insert(
      collection,
      json!({"id": "atomic-1", "name": "Second", "score": 2}),
    )
    .await;
  assert!(
    rejected.is_err(),
    "a duplicate id must be rejected, got {rejected:?}"
  );
  assert_eq!(
    provider.count(collection, None).await?,
    before,
    "a rejected insert must not change the row count"
  );
  let survivor = provider
    .find_by_id(collection, "atomic-1")
    .await?
    .ok_or_else(|| OrmError::NotFound(format!("{}/atomic-1", collection)))?;
  assert_eq!(
    survivor["name"], "First",
    "the rejected insert must not have overwritten the row"
  );
  Ok(())
}

// The honest answer from a provider that has no transactions: every entry
// point fails with NotSupported, and a forged id can neither commit nor
// roll anything back.
async fn assert_no_transaction_surface<P>(provider: &P) -> OrmResult<()>
where
  P: Storage + TransactionControl,
{
  assert!(
    !provider.supports(Capability::Transactions),
    "a provider whose transaction entry points are NotSupported must not advertise the capability"
  );
  let begun = provider.begin_transaction().await;
  assert!(
    matches!(&begun, Err(OrmError::NotSupported(_))),
    "begin_transaction must be NotSupported, got {begun:?}"
  );
  assert!(!provider.is_transaction_active().await?);

  let forged = TransactionId::new("forged-transaction");
  assert!(
    provider.commit_transaction(forged.clone()).await.is_err(),
    "committing an id that was never begun must fail"
  );
  assert!(
    provider.rollback_transaction(forged).await.is_err(),
    "rolling back an id that was never begun must fail"
  );
  Ok(())
}

// The transactional answer: a transaction that fails mid-way leaves no
// partial writes, and a commit makes its writes durable.
//
// Only SQLite claims the capability today, so without `sql-sqlite` no
// caller exists and the helper is compiled out of the lint rather than
// left as a warning.
#[cfg_attr(not(feature = "sql-sqlite"), allow(dead_code))]
async fn assert_rollback_leaves_no_partial_writes<P>(
  provider: &P,
  collection: &str,
) -> OrmResult<()>
where
  P: Storage + TransactionControl,
{
  assert!(
    provider.supports(Capability::Transactions),
    "a provider that rolls back must advertise the Transactions capability"
  );

  // The state the transaction has to return to: one row that survives it.
  provider
    .insert(
      collection,
      json!({"id": "keep", "name": "Keep", "score": 0}),
    )
    .await?;
  let baseline = provider.count(collection, None).await?;

  let tx = provider.begin_transaction().await?;
  assert!(provider.is_transaction_active().await?);

  // Two writes land inside the transaction…
  provider
    .insert(collection, json!({"id": "tx-1", "name": "One", "score": 1}))
    .await?;
  provider
    .insert(collection, json!({"id": "tx-2", "name": "Two", "score": 2}))
    .await?;
  assert_eq!(
    provider.count(collection, None).await?,
    baseline + 2,
    "the open transaction's writes are visible on the same connection"
  );

  // …then one fails: tx-1 already exists. The failure must not end the
  // transaction and must not write anything by itself.
  let midway = provider
    .insert(collection, json!({"id": "tx-1", "name": "Dup", "score": 9}))
    .await;
  assert!(
    midway.is_err(),
    "a duplicate id inside the transaction must be rejected, got {midway:?}"
  );
  assert!(provider.is_transaction_active().await?);
  assert_eq!(provider.count(collection, None).await?, baseline + 2);

  // A second transaction cannot open while one is active, and a forged id
  // cannot roll the real one back.
  assert!(provider.begin_transaction().await.is_err());
  let forged = TransactionId::new("forged-transaction");
  assert!(provider.rollback_transaction(forged).await.is_err());
  assert!(provider.is_transaction_active().await?);

  // Rollback: both writes are gone, the pre-transaction row is intact.
  provider.rollback_transaction(tx.clone()).await?;
  assert!(!provider.is_transaction_active().await?);
  assert_eq!(provider.count(collection, None).await?, baseline);
  assert!(provider.find_by_id(collection, "tx-1").await?.is_none());
  assert!(provider.find_by_id(collection, "tx-2").await?.is_none());
  assert!(provider.find_by_id(collection, "keep").await?.is_some());

  // Commit: the write survives it, and the finished id can be neither
  // committed nor rolled back a second time.
  let tx = provider.begin_transaction().await?;
  provider
    .insert(
      collection,
      json!({"id": "tx-3", "name": "Three", "score": 3}),
    )
    .await?;
  provider.commit_transaction(tx.clone()).await?;
  assert!(!provider.is_transaction_active().await?);
  assert_eq!(provider.count(collection, None).await?, baseline + 1);
  assert!(provider.find_by_id(collection, "tx-3").await?.is_some());
  assert!(
    provider.commit_transaction(tx.clone()).await.is_err(),
    "a committed id must not commit twice"
  );
  assert!(
    provider.rollback_transaction(tx).await.is_err(),
    "a committed id must not roll back"
  );

  Ok(())
}

// Every document the contract moves carries a string id; one that does not
// is an error, never a silently skipped row.
fn ids(docs: &[Value]) -> OrmResult<Vec<String>> {
  docs
    .iter()
    .map(|doc| {
      doc["id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| OrmError::InvalidQuery(format!("document without a string id: {}", doc)))
    })
    .collect()
}

#[cfg(feature = "json")]
mod json_backend {
  use super::*;
  use nosql_orm::providers::JsonProvider;

  async fn provider() -> OrmResult<(JsonProvider, tempfile::TempDir)> {
    let temp = tempfile::tempdir()?;
    let provider = JsonProvider::new(temp.path()).await?;
    Ok((provider, temp))
  }

  #[tokio::test]
  async fn json_satisfies_the_storage_contract() -> OrmResult<()> {
    let (provider, _temp) = provider().await?;
    assert_storage_contract(&provider, "json_docs").await?;
    assert_collection_is_listed(&provider, "json_docs").await?;
    assert_failed_write_leaves_no_partial_writes(&provider, "json_docs").await?;
    assert_no_transaction_surface(&provider).await
  }

  #[tokio::test]
  async fn json_backend_bridge_satisfies_the_storage_contract() -> OrmResult<()> {
    let (provider, _temp) = provider().await?;
    let bridge: DynBackend = std::sync::Arc::new(provider);
    assert_storage_contract(&bridge, "json_bridge").await
  }
}

#[cfg(feature = "sql-sqlite")]
mod sqlite_backend {
  use super::*;
  use nosql_orm::provider::AdminCommands;
  use nosql_orm::providers::sql::SqliteProvider;

  // Two tables: one for the concrete provider, one for the dyn bridge, so
  // the two suites never see each other's rows. SQLite reserves every name
  // that starts with `sqlite_`, hence `conformance_` rather than `sqlite_`.
  const TABLES: [&str; 2] = ["conformance_docs", "conformance_bridge"];

  async fn provider() -> OrmResult<(SqliteProvider, tempfile::TempDir)> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("conformance.db");
    let path = path.to_str().ok_or_else(|| {
      OrmError::InvalidInput(format!(
        "tempdir path is not valid UTF-8: {}",
        path.display()
      ))
    })?;
    let provider = SqliteProvider::connect(path).await?;
    for table in TABLES {
      let sql = format!(
        "CREATE TABLE {} (id TEXT PRIMARY KEY, name TEXT, score INTEGER, tier TEXT)",
        table
      );
      provider.execute_raw(&sql, Vec::new()).await?;
    }
    Ok((provider, temp))
  }

  #[tokio::test]
  async fn sqlite_satisfies_the_storage_contract() -> OrmResult<()> {
    let (provider, _temp) = provider().await?;
    assert_storage_contract(&provider, "conformance_docs").await?;
    assert_collection_is_listed(&provider, "conformance_docs").await?;
    assert_failed_write_leaves_no_partial_writes(&provider, "conformance_docs").await?;
    Ok(())
  }

  #[tokio::test]
  async fn sqlite_backend_bridge_satisfies_the_storage_contract() -> OrmResult<()> {
    let (provider, _temp) = provider().await?;
    let bridge: DynBackend = std::sync::Arc::new(provider);
    assert_storage_contract(&bridge, "conformance_bridge").await
  }

  #[tokio::test]
  async fn sqlite_rolls_back_a_transaction_that_failed_midway() -> OrmResult<()> {
    let (provider, _temp) = provider().await?;
    assert_rollback_leaves_no_partial_writes(&provider, "conformance_docs").await
  }
}

// The pooled PostgreSQL provider cannot keep BEGIN and COMMIT on one
// connection, so it reports NotSupported: the suite proves the contract and
// the honesty of that answer, never a rollback it cannot make.
#[cfg(feature = "sql-postgres")]
mod postgres_backend {
  use super::*;
  use nosql_orm::provider::AdminCommands;
  use nosql_orm::providers::sql::PostgresProvider;

  const TABLES: [&str; 2] = ["postgres_docs", "postgres_bridge"];

  async fn provider() -> OrmResult<PostgresProvider> {
    let url = std::env::var("POSTGRES_CONNECTION")
      .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/test_db".to_string());
    let provider = PostgresProvider::connect(&url).await?;
    // Start from empty tables: rows a previous run left behind would break
    // every count the contract asserts.
    for table in TABLES {
      provider
        .execute_raw(&format!("DROP TABLE IF EXISTS {}", table), Vec::new())
        .await?;
      provider
        .execute_raw(
          &format!(
            "CREATE TABLE {} (id TEXT PRIMARY KEY, name TEXT, score INTEGER, tier TEXT)",
            table
          ),
          Vec::new(),
        )
        .await?;
    }
    Ok(provider)
  }

  #[ignore = "needs a running PostgreSQL — set POSTGRES_CONNECTION"]
  #[tokio::test]
  async fn postgres_satisfies_the_storage_contract() -> OrmResult<()> {
    let provider = match provider().await {
      Ok(provider) => provider,
      Err(_) => return Ok(()),
    };
    assert_storage_contract(&provider, "postgres_docs").await?;
    assert_collection_is_listed(&provider, "postgres_docs").await?;
    assert_failed_write_leaves_no_partial_writes(&provider, "postgres_docs").await?;
    assert_no_transaction_surface(&provider).await
  }

  #[ignore = "needs a running PostgreSQL — set POSTGRES_CONNECTION"]
  #[tokio::test]
  async fn postgres_backend_bridge_satisfies_the_storage_contract() -> OrmResult<()> {
    let provider = match provider().await {
      Ok(provider) => provider,
      Err(_) => return Ok(()),
    };
    let bridge: DynBackend = std::sync::Arc::new(provider);
    assert_storage_contract(&bridge, "postgres_bridge").await
  }
}

#[cfg(feature = "sql-mysql")]
mod mysql_backend {
  use super::*;
  use nosql_orm::provider::AdminCommands;
  use nosql_orm::providers::sql::MySqlProvider;

  const TABLES: [&str; 2] = ["mysql_docs", "mysql_bridge"];

  async fn provider() -> OrmResult<MySqlProvider> {
    let url = std::env::var("MYSQL_CONNECTION")
      .unwrap_or_else(|_| "mysql://root:password@localhost:3306/test_db".to_string());
    let provider = MySqlProvider::connect(&url).await?;
    for table in TABLES {
      provider
        .execute_raw(&format!("DROP TABLE IF EXISTS {}", table), Vec::new())
        .await?;
      provider
        .execute_raw(
          &format!(
            "CREATE TABLE {} (id TEXT PRIMARY KEY, name TEXT, score INTEGER, tier TEXT)",
            table
          ),
          Vec::new(),
        )
        .await?;
    }
    Ok(provider)
  }

  #[ignore = "needs a running MySQL — set MYSQL_CONNECTION"]
  #[tokio::test]
  async fn mysql_satisfies_the_storage_contract() -> OrmResult<()> {
    let provider = match provider().await {
      Ok(provider) => provider,
      Err(_) => return Ok(()),
    };
    assert_storage_contract(&provider, "mysql_docs").await?;
    assert_collection_is_listed(&provider, "mysql_docs").await?;
    assert_failed_write_leaves_no_partial_writes(&provider, "mysql_docs").await?;
    assert_no_transaction_surface(&provider).await
  }

  #[ignore = "needs a running MySQL — set MYSQL_CONNECTION"]
  #[tokio::test]
  async fn mysql_backend_bridge_satisfies_the_storage_contract() -> OrmResult<()> {
    let provider = match provider().await {
      Ok(provider) => provider,
      Err(_) => return Ok(()),
    };
    let bridge: DynBackend = std::sync::Arc::new(provider);
    assert_storage_contract(&bridge, "mysql_bridge").await
  }
}

// MongoDB needs a session for real transactions, so the provider holds a
// Database and reports NotSupported rather than pretending a flag is one.
#[cfg(feature = "mongo")]
mod mongo_backend {
  use super::*;
  use nosql_orm::provider::AdminCommands;
  use nosql_orm::providers::mongo::MongoProvider;

  const COLLECTIONS: [&str; 2] = ["mongo_docs", "mongo_bridge"];

  async fn provider() -> OrmResult<MongoProvider> {
    let url =
      std::env::var("MONGO_CONNECTION").unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let provider = MongoProvider::connect(&url, "nosql_orm_conformance").await?;
    // The driver connects lazily: ping before the suite so an offline host
    // skips here instead of timing out server selection on every assertion.
    provider.health_check().await?;
    // Start from empty collections: documents a previous run left behind
    // would break every count the contract asserts.
    for collection in COLLECTIONS {
      provider.drop_collection(collection).await?;
    }
    Ok(provider)
  }

  #[ignore = "needs a running MongoDB — set MONGO_CONNECTION"]
  #[tokio::test]
  async fn mongo_satisfies_the_storage_contract() -> OrmResult<()> {
    let provider = match provider().await {
      Ok(provider) => provider,
      Err(_) => return Ok(()),
    };
    assert_storage_contract(&provider, "mongo_docs").await?;
    assert_collection_is_listed(&provider, "mongo_docs").await?;
    assert_failed_write_leaves_no_partial_writes(&provider, "mongo_docs").await?;
    assert_no_transaction_surface(&provider).await
  }

  #[ignore = "needs a running MongoDB — set MONGO_CONNECTION"]
  #[tokio::test]
  async fn mongo_backend_bridge_satisfies_the_storage_contract() -> OrmResult<()> {
    let provider = match provider().await {
      Ok(provider) => provider,
      Err(_) => return Ok(()),
    };
    let bridge: DynBackend = std::sync::Arc::new(provider);
    assert_storage_contract(&bridge, "mongo_bridge").await
  }
}

// Redis reports NotSupported for transactions too, and there is no failing
// write to probe: `SET` is last-write-wins, so a duplicate id overwrites
// instead of erroring, while every write Redis does make is a single
// command it executes atomically. The suite therefore proves the contract
// and the honesty of the transaction answer, and records why the
// partial-write probe does not apply here.
#[cfg(feature = "redis")]
mod redis_backend {
  use super::*;
  use nosql_orm::provider::AdminCommands;
  use nosql_orm::providers::redis::RedisProvider;

  const COLLECTIONS: [&str; 2] = ["redis_docs", "redis_bridge"];

  async fn provider() -> OrmResult<RedisProvider> {
    let url =
      std::env::var("REDIS_CONNECTION").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let provider = RedisProvider::new(&url).await?;
    // Start from empty collections: documents a previous run left behind
    // would break every count the contract asserts.
    for collection in COLLECTIONS {
      provider.drop_collection(collection).await?;
    }
    Ok(provider)
  }

  #[ignore = "needs a running Redis — set REDIS_CONNECTION"]
  #[tokio::test]
  async fn redis_satisfies_the_storage_contract() -> OrmResult<()> {
    let provider = match provider().await {
      Ok(provider) => provider,
      Err(_) => return Ok(()),
    };
    assert_storage_contract(&provider, "redis_docs").await?;
    assert_collection_is_listed(&provider, "redis_docs").await?;
    assert_no_transaction_surface(&provider).await
  }

  #[ignore = "needs a running Redis — set REDIS_CONNECTION"]
  #[tokio::test]
  async fn redis_backend_bridge_satisfies_the_storage_contract() -> OrmResult<()> {
    let provider = match provider().await {
      Ok(provider) => provider,
      Err(_) => return Ok(()),
    };
    let bridge: DynBackend = std::sync::Arc::new(provider);
    assert_storage_contract(&bridge, "redis_bridge").await
  }
}
