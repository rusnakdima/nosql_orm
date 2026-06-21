use mongodb::bson::{doc, to_bson, Bson, Document};

use crate::error::{OrmError, OrmResult};
use crate::providers::mongo::helpers::regex_escape;
use crate::query::Filter;

const MAX_PATTERN_LENGTH: usize = 500;
const MAX_REPEAT_COUNT: usize = 1000;

fn validate_regex_pattern(pattern: &str) -> OrmResult<()> {
  if pattern.len() > MAX_PATTERN_LENGTH {
    return Err(OrmError::InvalidInput(format!(
      "Regex pattern exceeds maximum length of {}",
      MAX_PATTERN_LENGTH
    )));
  }
  let repeat_count = pattern.matches("{/").count();
  if repeat_count > MAX_REPEAT_COUNT {
    return Err(OrmError::InvalidInput(format!(
      "Regex pattern contains too many repetition quantifiers (max {})",
      MAX_REPEAT_COUNT
    )));
  }
  if pattern.contains("(?")
    || pattern.contains("(?=")
    || pattern.contains("(?!")
    || pattern.contains("(?<=")
    || pattern.contains("(?<!")
  {
    return Err(OrmError::InvalidInput(
      "Regex lookahead/lookbehind assertions not allowed".to_string(),
    ));
  }
  Ok(())
}

pub fn filter_to_doc(filter: &Filter) -> OrmResult<Document> {
  match filter {
    Filter::Eq(f, v) => Ok(doc! { f: to_bson(v).unwrap_or(Bson::Null) }),
    Filter::Ne(f, v) => Ok(doc! { f: { "$ne": to_bson(v).unwrap_or(Bson::Null) } }),
    Filter::Gt(f, v) => Ok(doc! { f: { "$gt": to_bson(v).unwrap_or(Bson::Null) } }),
    Filter::Gte(f, v) => Ok(doc! { f: { "$gte": to_bson(v).unwrap_or(Bson::Null) } }),
    Filter::Lt(f, v) => Ok(doc! { f: { "$lt": to_bson(v).unwrap_or(Bson::Null) } }),
    Filter::Lte(f, v) => Ok(doc! { f: { "$lte": to_bson(v).unwrap_or(Bson::Null) } }),
    Filter::In(f, vals) => {
      let bson_vals: Vec<Bson> = vals
        .iter()
        .map(|v| to_bson(v).unwrap_or(Bson::Null))
        .collect();
      Ok(doc! { f: { "$in": bson_vals } })
    }
    Filter::NotIn(f, vals) => {
      let bson_vals: Vec<Bson> = vals
        .iter()
        .map(|v| to_bson(v).unwrap_or(Bson::Null))
        .collect();
      Ok(doc! { f: { "$nin": bson_vals } })
    }
    Filter::ArrayContains(f, v) => Ok(
      doc! { f: { "$elemMatch": { "$eq": to_bson(v).unwrap_or(Bson::Null) } } },
    ),
    Filter::Contains(f, sub) => {
      validate_regex_pattern(sub)?;
      Ok(doc! { f: { "$regex": sub, "$options": "i" } })
    }
    Filter::StartsWith(f, prefix) => {
      let escaped = regex_escape(prefix);
      Ok(doc! { f: { "$regex": format!("^{}", escaped), "$options": "i" } })
    }
    Filter::And(filters) => {
      let mut docs = Vec::new();
      for f in filters {
        docs.push(Bson::Document(filter_to_doc(f)?));
      }
      Ok(doc! { "$and": docs })
    }
    Filter::Or(filters) => {
      let mut docs = Vec::new();
      for f in filters {
        docs.push(Bson::Document(filter_to_doc(f)?));
      }
      Ok(doc! { "$or": docs })
    }
    Filter::Not(inner) => Ok(doc! { "$nor": [filter_to_doc(inner)?] }),
    Filter::IsNull(f) => Ok(doc! { f: { "$exists": false } }),
    Filter::IsNotNull(f) => Ok(doc! { f: { "$exists": true, "$ne": Bson::Null } }),
    Filter::Like(f, pattern) => {
      validate_regex_pattern(pattern)?;
      Ok(doc! { f: { "$regex": pattern, "$options": "i" } })
    }
    Filter::EndsWith(f, suffix) => {
      let escaped = regex_escape(suffix);
      Ok(doc! { f: { "$regex": format!(".*{}$", escaped), "$options": "i" } })
    }
    Filter::Between(f, min, max) => Ok(
      doc! { f: { "$gte": to_bson(min).unwrap_or(Bson::Null), "$lte": to_bson(max).unwrap_or(Bson::Null) } },
    ),
  }
}
