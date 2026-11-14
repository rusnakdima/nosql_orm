# New SQL database providers (coming soon)

> Status: **coming soon** — planned for 0.11.0–0.12.0. Not implemented in 0.9.0.

Extend the SQL provider family beyond Postgres/MySQL/SQLite.

---

## Planned providers

| Database | Feature flag | Notes |
|---|---|---|
| SQL Server (MSSQL) | `sql-mssql` | Enterprise; TDS protocol via `tiberius` |
| Oracle | `sql-oracle` | Enterprise; OCI or thin driver |
| CockroachDB | `sql-cockroach` | Postgres wire protocol; reuse `tokio-postgres` with Cockroach-specific DDL |
| DuckDB | `sql-duckdb` | Embedded analytical; `duckdb` crate |
| ClickHouse | `sql-clickhouse` | Columnar analytics; HTTP/native protocol |
| MariaDB | `sql-mariadb` | MySQL-compatible; reuse `mysql_async` with MariaDB DDL tweaks |

## Planned work

- Add a `SqlDialect` trait so DDL/DML differences (IDENTITY vs AUTOINCREMENT,
  LIMIT vs TOP, JSON functions) are isolated per provider.
- Reuse the existing `sql-postgres`/`sql-mysql`/`sql-sqlite` code paths where
  wire protocols overlap (CockroachDB ≈ Postgres, MariaDB ≈ MySQL).
- Add integration tests per provider (gated by feature flags, like existing
  `test_postgres_integration`).

## Why coming soon

Each provider needs a driver crate, dialect quirks and CI infrastructure.
Tracked as `nosql_orm` roadmap item.

## See also

- [03-provider.md](../03-provider.md) — providers
- [07r-sql.md](07r-sql.md) — SQL utilities
