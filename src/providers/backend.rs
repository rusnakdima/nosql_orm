//! Connecting to a storage backend by name.
//!
//! An application stores a connection as a *kind* (`postgres`, `sqlite`, …)
//! and a *url*, and asks the library to open it. This module owns that
//! mapping, so the application never names a provider type.

use crate::error::{OrmError, OrmResult};
use crate::provider::StorageBackend;
use std::sync::Arc;

/// The backends a connection may address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
  /// A directory of JSON files.
  Json,
  /// A SQLite database file.
  Sqlite,
  /// A PostgreSQL server.
  Postgres,
  /// A MySQL server.
  MySql,
  /// A MongoDB server.
  Mongo,
  /// A Redis server.
  Redis,
}

impl BackendKind {
  /// Parse a backend name.
  ///
  /// The names are the ones an application stores; the aliases cover the
  /// spellings the same backend is known by.
  pub fn parse(name: &str) -> Option<Self> {
    match name.trim().to_ascii_lowercase().as_str() {
      "json" | "file" | "local" => Some(Self::Json),
      "sqlite" | "sqlite3" => Some(Self::Sqlite),
      "postgres" | "postgresql" | "pg" => Some(Self::Postgres),
      "mysql" | "mariadb" => Some(Self::MySql),
      "mongo" | "mongodb" => Some(Self::Mongo),
      "redis" => Some(Self::Redis),
      _ => None,
    }
  }

  /// The canonical name of the backend.
  pub fn name(self) -> &'static str {
    match self {
      Self::Json => "json",
      Self::Sqlite => "sqlite",
      Self::Postgres => "postgres",
      Self::MySql => "mysql",
      Self::Mongo => "mongodb",
      Self::Redis => "redis",
    }
  }
}

/// The parts of a connection, as a form collects them.
///
/// A form that asks for a host, a port, a database and a user gives the
/// library the pieces; the library knows how to fold them into the address
/// its backend expects.
#[derive(Debug, Clone, Default)]
pub struct ConnectionParts {
  /// The host the server runs on.
  pub host: String,
  /// The port the server listens on.
  pub port: Option<u16>,
  /// The database (or file) to work on.
  pub database: String,
  /// The user to authenticate as.
  pub user: String,
  /// The password to authenticate with.
  pub password: String,
}

/// The parts of an address, in the order a URL writes them.
fn split_user_password(parts: &ConnectionParts) -> Option<String> {
  if parts.user.is_empty() {
    return None;
  }
  let mut credentials = parts.user.clone();
  if !parts.password.is_empty() {
    credentials.push(':');
    credentials.push_str(&parts.password);
  }
  Some(credentials)
}

impl BackendKind {
  /// The scheme a connection of this kind is written with.
  pub fn scheme(self) -> &'static str {
    match self {
      Self::Json => "",
      Self::Sqlite => "",
      Self::Postgres => "postgres://",
      Self::MySql => "mysql://",
      Self::Mongo => "mongodb://",
      Self::Redis => "redis://",
    }
  }

  /// The port a connection of this kind uses when the form leaves it out.
  pub fn default_port(self) -> Option<u16> {
    match self {
      Self::Postgres => Some(5432),
      Self::MySql => Some(3306),
      Self::Mongo => Some(27017),
      Self::Redis => Some(6379),
      Self::Json | Self::Sqlite => None,
    }
  }

  /// Fold the parts of a connection into the address its backend expects.
  pub fn address(self, parts: &ConnectionParts) -> String {
    match self {
      // A file-backed backend is addressed by the path itself.
      Self::Json => parts.database.clone(),
      Self::Sqlite => {
        if parts.database.is_empty() {
          parts.host.clone()
        } else {
          parts.database.clone()
        }
      }
      _ => {
        let mut address = String::from(self.scheme());
        if let Some(credentials) = split_user_password(parts) {
          address.push_str(&credentials);
          address.push('@');
        }
        address.push_str(if parts.host.is_empty() {
          "localhost"
        } else {
          &parts.host
        });
        let port = parts.port.or(self.default_port());
        if let Some(port) = port {
          address.push(':');
          address.push_str(&port.to_string());
        }
        if !parts.database.is_empty() {
          address.push('/');
          address.push_str(&parts.database);
        }
        address
      }
    }
  }
}

/// Open a backend and return it as a uniform handle.
///
/// The `url` is the address the backend is reached at: a directory for
/// `json`, a file path for `sqlite`, and a connection string for the servers.
pub async fn connect_backend(kind: BackendKind, url: &str) -> OrmResult<Arc<dyn StorageBackend>> {
  match kind {
    BackendKind::Json => {
      #[cfg(feature = "json")]
      {
        let provider = crate::providers::json::JsonProvider::new(url).await?;
        Ok(Arc::new(provider))
      }
      #[cfg(not(feature = "json"))]
      {
        Err(missing_feature("json"))
      }
    }
    BackendKind::Sqlite => {
      #[cfg(feature = "sql-sqlite")]
      {
        let path = strip_scheme(url, "sqlite://");
        let provider = crate::providers::sql::SqliteProvider::connect(path).await?;
        Ok(Arc::new(provider))
      }
      #[cfg(not(feature = "sql-sqlite"))]
      {
        Err(missing_feature("sqlite"))
      }
    }
    BackendKind::Postgres => {
      #[cfg(feature = "sql-postgres")]
      {
        let provider = crate::providers::sql::PostgresProvider::connect(url).await?;
        Ok(Arc::new(provider))
      }
      #[cfg(not(feature = "sql-postgres"))]
      {
        Err(missing_feature("postgres"))
      }
    }
    BackendKind::MySql => {
      #[cfg(feature = "sql-mysql")]
      {
        let provider = crate::providers::sql::MySqlProvider::connect(url).await?;
        Ok(Arc::new(provider))
      }
      #[cfg(not(feature = "sql-mysql"))]
      {
        Err(missing_feature("mysql"))
      }
    }
    BackendKind::Mongo => {
      #[cfg(feature = "mongo")]
      {
        let database = database_from_url(url).unwrap_or_else(|| "nosql_orm".to_string());
        let provider = crate::providers::mongo::MongoProvider::connect(url, database).await?;
        Ok(Arc::new(provider))
      }
      #[cfg(not(feature = "mongo"))]
      {
        Err(missing_feature("mongodb"))
      }
    }
    BackendKind::Redis => {
      #[cfg(feature = "redis")]
      {
        let provider = crate::providers::redis::RedisProvider::new(url).await?;
        Ok(Arc::new(provider))
      }
      #[cfg(not(feature = "redis"))]
      {
        Err(missing_feature("redis"))
      }
    }
  }
}

/// The error a backend reports when the library was built without it.
#[allow(dead_code)]
fn missing_feature(kind: &str) -> OrmError {
  OrmError::NotSupported(format!(
    "the {} backend is not available: enable the matching nosql_orm feature",
    kind
  ))
}

/// Open a backend by name.
///
/// An unknown name is an error, not a silent fallback: a connection that
/// points at a backend the library cannot open must not look like it works.
pub async fn connect_by_name(name: &str, url: &str) -> OrmResult<Arc<dyn StorageBackend>> {
  let kind = BackendKind::parse(name)
    .ok_or_else(|| OrmError::InvalidInput(format!("unknown backend: {}", name)))?;
  connect_backend(kind, url).await
}

/// Remove a scheme prefix from an address.
///
/// Only the SQLite address carries a scheme, so the helper lives with the
/// feature that uses it.
#[cfg(any(feature = "sql-sqlite", test))]
fn strip_scheme<'a>(url: &'a str, scheme: &'a str) -> &'a str {
  url.strip_prefix(scheme).unwrap_or(url)
}

/// Read the database name out of a connection string.
///
/// The name is the first segment of the path: `mongodb://host/orders` names
/// the database `orders`. Only the Mongo address names a database, so the
/// helper lives with the feature that uses it.
#[cfg(any(feature = "mongo", test))]
fn database_from_url(url: &str) -> Option<String> {
  let path = url.split_once("://").map(|(_, rest)| rest)?;
  let name = path.split('/').nth(1)?.split('?').next()?;
  if name.is_empty() {
    None
  } else {
    Some(name.to_string())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_backend_name_parses_to_its_kind() {
    assert_eq!(
      BackendKind::parse("PostgreSQL"),
      Some(BackendKind::Postgres)
    );
    assert_eq!(BackendKind::parse("mongo"), Some(BackendKind::Mongo));
    assert_eq!(BackendKind::parse("sqlite"), Some(BackendKind::Sqlite));
    assert_eq!(BackendKind::parse("memcached"), None);
  }

  #[test]
  fn the_database_name_comes_from_the_path() {
    assert_eq!(
      database_from_url("mongodb://localhost:27017/orders"),
      Some("orders".to_string())
    );
    assert_eq!(database_from_url("mongodb://localhost:27017"), None);
  }

  #[test]
  fn a_sqlite_scheme_is_stripped() {
    assert_eq!(strip_scheme("sqlite:///tmp/a.db", "sqlite://"), "/tmp/a.db");
    assert_eq!(strip_scheme("/tmp/a.db", "sqlite://"), "/tmp/a.db");
  }
}
