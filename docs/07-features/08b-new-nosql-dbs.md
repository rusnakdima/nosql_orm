# New NoSQL database providers

> Status: **partially implemented natively** — 0.10.0+.
> etcd, InfluxDB, CouchDB, DynamoDB and Elasticsearch run on their own drivers;
> the rest are still aliases over an existing provider, with native drivers pending.

Extend the NoSQL provider family beyond MongoDB/Redis.

---

## Providers

| Database | Feature flag | Implementation |
|---|---|---|
| etcd | `etcd` | ✅ **native** — `etcd-client` (gRPC) |
| InfluxDB | `influxdb` | ✅ **native** — `influxdb` (HTTP) |
| CouchDB | `couchdb` | ✅ **native** — `reqwest` (HTTP) |
| Couchbase | `couchbase` | 🟡 alias over `MongoProvider` |
| DynamoDB | `dynamodb` | ✅ **native** — `reqwest` (AWS JSON + SigV4) |
| Firestore | `firestore` | 🟡 alias over `MongoProvider` |
| Elasticsearch / OpenSearch | `elasticsearch` | ✅ **native** — `reqwest` (HTTP) |
| Neo4j | `neo4j` | 🟡 alias over `MongoProvider` |
| InfluxDB | `influxdb` | ✅ **native** — `influxdb` (HTTP) |
| Cassandra / ScyllaDB | `cassandra` | 🟡 alias over `MongoProvider` |

An alias exposes the full `Storage`, `SchemaIntrospection` and
`AdminCommands` implementation of the provider it reuses, so the same
contract (filters, pagination, sorting) applies either way. What an alias
does **not** give you is the target product's own wire protocol, query
language or consistency model — that is what the pending native drivers add.

---

## etcd (native)

`EtcdProvider` speaks etcd's gRPC API directly through `etcd-client`.

```rust
use nosql_orm::providers::etcd::EtcdProvider;

// one endpoint, or several comma-separated
let provider = EtcdProvider::connect("http://127.0.0.1:2379").await?;
// or from a ProviderConfig (database = key prefix)
let provider = EtcdProvider::from_config(&config).await?;
// isolate namespaces by key prefix
let provider = provider.with_prefix("myapp/");
```

**Key layout.** A collection is a key prefix and a document is one key:
`{prefix}{collection}/{id}`. Because `/` is rejected in collection names the
prefix is unambiguous, so a single range read returns a whole collection —
no id-set bookkeeping the way `RedisProvider` needs it. A collection name
containing `/` is refused with `InvalidInput`.

**What is real:**

- `insert` is genuine insert-if-absent (etcd `Txn` compare on
  `create_revision == 0`); a duplicate id returns `Duplicate` and leaves the
  existing row untouched.
- `update` requires an existing document; replacing a missing one returns
  `NotFound` rather than silently inserting.
- `delete` reports whether the key existed, from one atomic round trip.
- `rename_collection` moves every document and drops the old prefix.
- `list_collections` / `describe_collection` / `get_collection_stats` derive
  counts, field names and sizes from the keys themselves.

**Honest limits:**

- `create_index` / `drop_index` / `list_indexes` → `NotSupported` (no index
  engine), so `Capability::Indexes` is not advertised.
- `execute_raw` → `NotSupported` (no query language); `aggregate` inherits
  the trait default `NotSupported`.
- `TransactionControl` → `NotSupported`. etcd's `Txn` is a single atomic
  request, not a session spanning requests, so there is no handle to
  begin/commit/rollback and no id is handed out that cannot undo anything.

**Tests:** `tests/test_etcd_integration.rs` (9 tests) runs the storage
contract, duplicate rejection, update-requires-existence, bulk counts,
introspection, capability honesty, transaction honesty, name validation and
rename. It is `#[ignore]`d and skips when no server is reachable:

```bash
ETCD_ENDPOINTS=http://127.0.0.1:2379 \
  cargo test --features etcd --test test_etcd_integration -- --include-ignored
```

---

## InfluxDB (native)

`InfluxDbProvider` speaks InfluxDB's HTTP API through the `influxdb` crate.

```rust
use nosql_orm::providers::influxdb::InfluxDbProvider;

// the database is created if missing (CREATE DATABASE is idempotent)
let provider = InfluxDbProvider::connect("http://127.0.0.1:8086", "mydb").await?;
// or from a ProviderConfig, with `options` carrying username/password or token
let provider = InfluxDbProvider::from_config(&config).await?;
```

**Mapping onto a time-series store:**

| Document store | InfluxDB |
|---|---|
| collection | measurement |
| document id | tag `id` (indexed, so `WHERE id=…` is cheap) |
| document | field `doc`, the whole document as one JSON string |

The document lives in a single string field on purpose: a field is typed once
per measurement in InfluxDB, so a JSON blob sidesteps field-type conflicts and
carries arbitrary nested data. Filtering, sorting and pagination therefore
happen in memory after the read, exactly as `RedisProvider` and
`EtcdProvider` do.

**The one real mismatch:** InfluxDB has no unique key, so writing an existing
id *appends* a second point instead of replacing it. Every read and every
count therefore goes through `LAST(doc) … GROUP BY id`, which keeps the newest
point per id and restores "one row per id". `count` and `find_many` are
asserted to agree after an update, because that is where a naive point count
would drift.

**What is real:**

- `insert` / `update` check existence first, so a duplicate id returns
  `Duplicate` and a missing one returns `NotFound` rather than inserting.
  (etcd could do this atomically with `Txn`; InfluxDB has no conditional
  write, so this is a check-then-write — documented, not papered over.)
- `delete` reports whether the document existed, then drops every point for
  that id.
- `execute_raw` genuinely runs **InfluxQL** — `Capability::RawQueries` is
  advertised because it works. A `GROUP BY` returns one series per tag set
  and all of them are gathered, not just the first.
- Measurement names are double-quoted and ids are `quote_literal`-escaped, so
  a name like `my-meas.1` or an id containing quotes round-trips.

**Honest limits:**

- `create_index` / `list_indexes` → `NotSupported` (InfluxDB indexes its own
  series), so `Capability::Indexes` is not advertised.
- `aggregate` → `NotSupported`: it takes a Mongo-style pipeline, which
  InfluxQL cannot execute.
- `TransactionControl` → `NotSupported`: every write is applied on arrival,
  with no session to begin/commit/rollback.

**Tests:** `tests/test_influxdb_integration.rs` (11 tests) covers the storage
contract, count/read dedup after updates, duplicate rejection,
update-requires-existence, introspection, `execute_raw`, capability honesty,
measurement-name quoting, id-escaping, rename and name validation. It is
`#[ignore]`d and skips when no server answers:

```bash
cargo test --features influxdb --test test_influxdb_integration -- --include-ignored
```

Unit tests for the escaping helpers live in
`src/providers/influxdb/tests/mod.rs` — that is where the `"`-in-tag-value
regression is pinned down.

## See also

- [03-provider.md](../03-provider.md) — providers
- [07n-cdc.md](07n-cdc.md) — change data capture
- [07p-subscription.md](07p-subscription.md) — real-time subscriptions
