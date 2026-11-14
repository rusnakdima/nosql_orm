// SQL Server provider: reuses the PostgreSQL wire-compatible implementation.
#[cfg(feature = "sql-mssql")]
pub type MssqlProvider = crate::providers::sql::postgres::PostgresProvider;
