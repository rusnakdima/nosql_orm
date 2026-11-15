use crate::error::OrmResult;
use crate::providers::couchdb::CouchDbProvider;

#[ignore = "needs a running CouchDB"]
#[tokio::test]
async fn couchdb_insert_requires_live_server() -> OrmResult<()> {
  let p = CouchDbProvider::connect("http://127.0.0.1:5984").await?;
  Ok(())
}

#[test]
fn module_compiles() {
  let _ = std::mem::size_of::<CouchDbProvider>();
}
