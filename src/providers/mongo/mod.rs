use crate::error::{map_err_connection, OrmError, OrmResult};
use crate::nosql_index::{NosqlIndex, NosqlIndexInfo};
use crate::provider::{
  AdminCommands, Capability, CollectionMeta, CollectionSchema, CollectionStats, ConnectionHealth,
  FieldInfo, IndexInfo, ProviderCapabilities, ProviderConfig, RawResult, SchemaIntrospection,
  Storage, TransactionControl, TransactionId,
};
use crate::query::Filter;
use crate::utils::generate_id;
use async_trait::async_trait;
use mongodb::{
  bson::{doc, Bson, Document},
  options::FindOptions,
  Database,
};
use serde_json::Value;
use std::collections::HashMap;

mod convert;
mod filter;
mod helpers;
mod indexes;

use crate::providers::mongo::convert::{bson_to_json, json_to_bson};
use crate::providers::mongo::filter::filter_to_doc;
use crate::providers::mongo::indexes::{build_index_keys, build_index_options};


mod provider;
pub use provider::*;

#[cfg(test)]
mod tests;
