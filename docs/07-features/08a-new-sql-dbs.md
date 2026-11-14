# New SQL database providers

> Status: **implemented** — 0.10.0+.
> SQL Server (`tiberius`), Oracle (`oracle`/ODPI-C), DuckDB (`duckdb`) and
> ClickHouse (`clickhouse`) run on their own native drivers; CockroachDB and
> MariaDB remain wire-compatible aliases of the Postgres/MySQL providers.

Extend the SQL provider family beyond Postgres/MySQL/SQLite.

---

## Providers

| Database | Feature flag | Implementation |
|---|---|---|
| SQL Server (MSSQL) | `sql-mssql` | `MssqlProvider` on the `tiberius` TDS driver (native) |
| Oracle | `sql-oracle` | `OracleProvider` on the `oracle` crate (native, ODPI-C) |
| CockroachDB | `sql-cockroach` | Reuses `PostgresProvider` (same wire protocol) |
| DuckDB | `sql-duckdb` | `DuckDbProvider` on the `duckdb` crate (native, embedded) |
| ClickHouse | `sql-clickhouse` | `ClickHouseProvider` on the `clickhouse` HTTP crate (native) |
| MariaDB | `sql-mariadb` | Reuses `MySqlProvider` (same wire protocol) |

Each provider exposes the full `Storage`, `SchemaIntrospection`,
`AdminCommands` and `TransactionControl` implementations, so every
feature-gated provider satisfies the same contract as
Postgres/MySQL/SQLite.

## Native driver notes

What each driver translates, because none of them speaks the SQL the shared
builders emit verbatim:

- **SQL Server** — one TDS connection per provider, so transactions share the
  session. `?` becomes `@P1…`, `TRUE`/`FALSE` become `1`/`0`, and
  `LIMIT n OFFSET m` becomes `OFFSET … ROWS FETCH NEXT … ROWS ONLY` (a
  constant `ORDER BY` is synthesized when the statement had none). TLS goes
  through `native-tls`, so a local container typically needs
  `TrustServerCertificate=true` in the ADO connection string. `drop_index`
  reports `NotSupported`: the shared statement carries no table name, which
  T-SQL requires. Connection string shape:
  `server=tcp:localhost,1433;user id=sa;password=…;TrustServerCertificate=true`.
- **Oracle** — the driver vendors ODPI-C, so the crate builds on a machine
  with no Oracle software; the Oracle Client library is loaded when
  `connect` runs (DSN shape `user/password@//host:1521/service`). Booleans
  travel as `NUMBER(1)`, the generated DDL quotes its key column so case
  round-trips against the quoted reads, writes auto-commit outside an
  explicit transaction, and `DROP … IF EXISTS` is stripped (23c syntax).
- **DuckDB** — embedded, one connection per provider; introspection reads
  `duckdb_tables()`, `duckdb_columns()` and `duckdb_indexes()`.
- **ClickHouse** — HTTP, no interactive transactions:
  `begin_transaction` reports `NotSupported` honestly. `UPDATE` is
  rewritten to an `ALTER TABLE … UPDATE` mutation (row counts are read
  before the mutation runs, since the HTTP interface does not report them),
  `CREATE INDEX` becomes a `minmax` skip index (a `UNIQUE` request is
  refused rather than downgraded), and `CREATE TABLE` gains a MergeTree
  engine. Tests are `#[ignore]`d without a server.

## See also

- [03-provider.md](../03-provider.md) — providers
- [07r-sql.md](07r-sql.md) — SQL utilities
