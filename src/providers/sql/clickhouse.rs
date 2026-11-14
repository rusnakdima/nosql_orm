// ClickHouse provider: reuses the PostgreSQL implementation as a baseline.
#[cfg(feature = "sql-clickhouse")]
pub type ClickHouseProvider = crate::providers::sql::postgres::PostgresProvider;
