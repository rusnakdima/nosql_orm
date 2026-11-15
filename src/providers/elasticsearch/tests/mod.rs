use crate::error::OrmResult;
use crate::providers::elasticsearch::ElasticsearchProvider;

#[ignore = "needs a running Elasticsearch"]
#[tokio::test]
async fn elasticsearch_insert_requires_live_server() -> OrmResult<()> {
  let p = ElasticsearchProvider::connect("http://127.0.0.1:9200").await?;
  Ok(())
}

#[test]
fn module_compiles() {
  let _ = std::mem::size_of::<ElasticsearchProvider>();
}
