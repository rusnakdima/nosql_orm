// Oracle provider: reuses the PostgreSQL implementation as a baseline.
#[cfg(feature = "sql-oracle")]
pub type OracleProvider = crate::providers::sql::postgres::PostgresProvider;
