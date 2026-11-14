// DuckDB provider: reuses the SQLite implementation (embedded).
#[cfg(feature = "sql-duckdb")]
pub type DuckDbProvider = crate::providers::sql::sqlite::SqliteProvider;
