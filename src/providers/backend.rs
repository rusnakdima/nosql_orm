use crate::error::{OrmError, OrmResult};
use crate::provider::StorageBackend;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
  Json,
  Sqlite,
  Postgres,
  MySql,
  Mongo,
  Redis,
}

impl BackendKind {
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

#[derive(Debug, Clone, Default)]
pub struct ConnectionParts {
  pub host: String,
  pub port: Option<u16>,
  pub database: String,
  pub user: String,
  pub password: String,
}

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

  pub fn default_port(self) -> Option<u16> {
    match self {
      Self::Postgres => Some(5432),
      Self::MySql => Some(3306),
      Self::Mongo => Some(27017),
      Self::Redis => Some(6379),
      Self::Json | Self::Sqlite => None,
    }
  }

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

// url is read only by the cfg branch for the chosen backend; without any
// backend feature every branch returns missing_feature and never touches it.
#[allow(unused_variables)]
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

#[allow(dead_code)]
fn missing_feature(kind: &str) -> OrmError {
  OrmError::NotSupported(format!(
    "the {} backend is not available: enable the matching nosql_orm feature",
    kind
  ))
}

pub async fn connect_by_name(name: &str, url: &str) -> OrmResult<Arc<dyn StorageBackend>> {
  let kind = BackendKind::parse(name)
    .ok_or_else(|| OrmError::InvalidInput(format!("unknown backend: {}", name)))?;
  connect_backend(kind, url).await
}

#[cfg(any(feature = "sql-sqlite", test))]
fn strip_scheme<'a>(url: &'a str, scheme: &'a str) -> &'a str {
  url.strip_prefix(scheme).unwrap_or(url)
}

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
mod tests;
