use mongodb::bson::{doc, to_bson, Bson, Document};

use crate::providers::mongo::helpers::regex_escape;
use crate::query::Filter;

pub fn filter_to_doc(filter: &Filter) -> Document {
  match filter {
    Filter::Eq(f, v) => doc! { f: to_bson(v).unwrap_or(Bson::Null) },
    Filter::Ne(f, v) => doc! { f: { "$ne": to_bson(v).unwrap_or(Bson::Null) } },
    Filter::Gt(f, v) => doc! { f: { "$gt": to_bson(v).unwrap_or(Bson::Null) } },
    Filter::Gte(f, v) => doc! { f: { "$gte": to_bson(v).unwrap_or(Bson::Null) } },
    Filter::Lt(f, v) => doc! { f: { "$lt": to_bson(v).unwrap_or(Bson::Null) } },
    Filter::Lte(f, v) => doc! { f: { "$lte": to_bson(v).unwrap_or(Bson::Null) } },
    Filter::In(f, vals) => {
      let bson_vals: Vec<Bson> = vals
        .iter()
        .map(|v| to_bson(v).unwrap_or(Bson::Null))
        .collect();
      doc! { f: { "$in": bson_vals } }
    }
    Filter::NotIn(f, vals) => {
      let bson_vals: Vec<Bson> = vals
        .iter()
        .map(|v| to_bson(v).unwrap_or(Bson::Null))
        .collect();
      doc! { f: { "$nin": bson_vals } }
    }
    Filter::Contains(f, sub) => doc! { f: { "$regex": sub, "$options": "i" } },
    Filter::StartsWith(f, prefix) => {
      doc! { f: { "$regex": format!("^{}", regex_escape(prefix)), "$options": "i" } }
    }
    Filter::And(filters) => {
      let docs: Vec<Bson> = filters
        .iter()
        .map(|f| Bson::Document(filter_to_doc(f)))
        .collect();
      doc! { "$and": docs }
    }
    Filter::Or(filters) => {
      let docs: Vec<Bson> = filters
        .iter()
        .map(|f| Bson::Document(filter_to_doc(f)))
        .collect();
      doc! { "$or": docs }
    }
    Filter::Not(inner) => doc! { "$nor": [filter_to_doc(inner)] },
    Filter::IsNull(f) => doc! { f: { "$exists": false } },
    Filter::IsNotNull(f) => doc! { f: { "$exists": true, "$ne": Bson::Null } },
    Filter::Like(f, pattern) => doc! { f: { "$regex": pattern, "$options": "i" } },
    Filter::EndsWith(f, suffix) => {
      let escaped = regex_escape(suffix);
      doc! { f: { "$regex": format!(".*{}$", escaped), "$options": "i" } }
    }
    Filter::Between(f, min, max) => {
      doc! { f: { "$gte": to_bson(min).unwrap_or(Bson::Null), "$lte": to_bson(max).unwrap_or(Bson::Null) } }
    }
  }
}
