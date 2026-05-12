use std::time::Duration;

use mongodb::{bson::Bson, options::IndexOptions};

use crate::error::OrmResult;
use crate::nosql_index::{NosqlIndex, NosqlIndexType};

use super::filter::filter_to_doc;

pub fn build_index_keys(index: &NosqlIndex) -> mongodb::bson::Document {
  use mongodb::bson::Document;
  let mut doc = Document::new();
  for (field, order) in index.get_fields() {
    let value: Bson = match index.get_index_type() {
      NosqlIndexType::Geospatial2dsphere => Bson::Int32(1),
      NosqlIndexType::Geospatial2d => Bson::Int32(1),
      NosqlIndexType::Text => Bson::String("text".to_string()),
      NosqlIndexType::Hashed => Bson::String("hashed".to_string()),
      _ => Bson::Int32(*order),
    };
    doc.insert(field, value);
  }
  doc
}

pub fn build_index_options(index: &NosqlIndex) -> OrmResult<IndexOptions> {
  use mongodb::bson::Document;
  let mut opts = IndexOptions::default();
  if let Some(name) = index.get_name() {
    opts.name = Some(name.to_string());
  }
  if index.is_unique() {
    opts.unique = Some(true);
  }
  if index.is_sparse() {
    opts.sparse = Some(true);
  }
  if let Some(ttl) = index.get_ttl_seconds() {
    opts.expire_after = Some(Duration::from_secs(ttl as u64));
  }
  if let Some(partial_filter) = index.get_partial_filter() {
    opts.partial_filter_expression = Some(filter_to_doc(partial_filter)?);
  }
  if let Some(weights) = index.get_weights() {
    let mut doc = Document::new();
    for (field, weight) in weights.iter() {
      doc.insert(field, *weight);
    }
    opts.weights = Some(doc);
  }
  if let Some(lang) = index.get_default_language() {
    opts.default_language = Some(lang.to_string());
  }
  Ok(opts)
}
