//! T9.9 / nosql_orm CI — verifies the json_tree refactor preserved storage
//! trait surface and removed unused id/base plumbing. Compile-coded pass/fail.
use nosql_orm::providers::json_tree::JsonTreeProvider;

#[test]
fn json_tree_provider_has_no_id_base_plumbing() {
  // Refactor contract: json_tree no longer hardcodes app identity or
  // id/base plumbing (e.g. "designer" hardcoded app id). Constructing
  // JsonTreeProvider must work with only a base directory path.
  let _provider = JsonTreeProvider::new("/tmp/ci-check-store");
}

#[test]
fn storage_trait_impl_for_json_tree() {
  // If the Storage / StorageBackend traits were broken by removing
  // id/base plumbing, this line fails to compile (trait impl missing).
  fn check<P: nosql_orm::provider::Storage + Clone + Send + Sync + 'static>() {}
  check::<JsonTreeProvider>();
}
