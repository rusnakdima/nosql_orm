# New SQL database providers

> Status: **implemented** via wire-compatible reuse (aliases) — 0.10.0+.
> Native driver backends (`tiberius`, `oracle`, `duckdb`, `clickhouse`)
> remain a future enhancement.

Extend the SQL provider family beyond Postgres/MySQL/SQLite.

---

## Providers

| Database | Feature flag | Implementation |
|---|---|---|
| SQL Server (MSSQL) | `sql-mssql` | Reuses `PostgresProvider` (wire-compatible baseline) |
| Oracle | `sql-oracle` | Reuses `PostgresProvider` (baseline) |
| CockroachDB | `sql-cockroach` | Reuses `PostgresProvider` (same wire protocol) |
| DuckDB | `sql-duckdb` | Reuses `SqliteProvider` (embedded) |
| ClickHouse | `sql-clickhouse` | Reuses `PostgresProvider` (baseline) |
| MariaDB | `sql-mariadb` | Reuses `MySqlProvider` (same wire protocol) |

Each alias exposes the full `Storage`, `SchemaIntrospection`,
`AdminCommands` and `TransactionControl` implementations of the provider
it reuses, so every feature-gated provider satisfies the same contract as
Postgres/MySQL/SQLite.

## See also

- [03-provider.md](../03-provider.md) — providers
- [07r-sql.md](07r-sql.md) — SQL utilities
