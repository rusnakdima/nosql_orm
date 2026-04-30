use mongodb::bson::{from_bson, to_bson, Bson, Document};
use serde_json::Value;

use crate::error::{OrmError, OrmResult};
use crate::providers::mongo::helpers::normalize_id;

pub fn json_to_bson(value: Value) -> OrmResult<Document> {
  let bson = to_bson(&value)
    .map_err(|e| OrmError::Serialization(serde::ser::Error::custom(e.to_string())))?;
  bson
    .as_document()
    .cloned()
    .ok_or_else(|| OrmError::Provider("Expected BSON document".to_string()))
}

pub fn bson_to_json(doc: Document) -> OrmResult<Value> {
  let bson = Bson::Document(doc);
  let json: Value = from_bson(bson)
    .map_err(|e| OrmError::Serialization(serde::ser::Error::custom(e.to_string())))?;
  Ok(normalize_id(json))
}
