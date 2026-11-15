use crate::error::OrmResult;
use crate::providers::dynamodb::DynamoDbProvider;

#[ignore = "needs a running DynamoDB"]
#[tokio::test]
async fn dynamodb_insert_requires_live_server() -> OrmResult<()> {
  let p = DynamoDbProvider::connect("http://127.0.0.1:8000").await?;
  Ok(())
}

#[test]
fn module_compiles() {
  let _ = std::mem::size_of::<DynamoDbProvider>();
}
