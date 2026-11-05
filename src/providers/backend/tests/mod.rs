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
