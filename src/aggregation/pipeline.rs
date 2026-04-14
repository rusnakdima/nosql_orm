use crate::error::OrmResult;
use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct AggregationPipeline {
  stages: Vec<Stage>,
}

impl AggregationPipeline {
  pub fn new() -> Self {
    Self { stages: Vec::new() }
  }

  pub fn match_stage(mut self, filter: Value) -> Self {
    self.stages.push(Stage::Match(filter));
    self
  }

  pub fn group<T: Into<Stage>>(mut self, stage: T) -> Self {
    self.stages.push(stage.into());
    self
  }

  pub fn sort(mut self, field: &str, ascending: bool) -> Self {
    self.stages.push(Stage::Sort {
      field: field.to_string(),
      ascending,
    });
    self
  }

  pub fn limit(mut self, n: u64) -> Self {
    self.stages.push(Stage::Limit(n));
    self
  }

  pub fn skip(mut self, n: u64) -> Self {
    self.stages.push(Stage::Skip(n));
    self
  }

  pub fn project(mut self, fields: Vec<(&str, Value)>) -> Self {
    let mut obj = serde_json::Map::new();
    for (k, v) in fields {
      obj.insert(k.to_string(), v);
    }
    self.stages.push(Stage::Project(Value::Object(obj)));
    self
  }

  pub fn build(&self) -> Vec<Value> {
    self.stages.iter().map(|s| s.to_value()).collect()
  }

  pub async fn execute<P: crate::provider::DatabaseProvider>(
    &self,
    provider: &P,
    collection: &str,
  ) -> OrmResult<Vec<Value>> {
    let all_docs = provider.find_all(collection).await?;
    let mut results = all_docs;

    for stage in &self.stages {
      results = stage.execute(results).await?;
    }

    Ok(results)
  }
}

#[derive(Debug, Clone)]
pub enum Stage {
  Match(Value),
  Group {
    id: Value,
    accumulators: std::collections::HashMap<String, Value>,
  },
  Sort {
    field: String,
    ascending: bool,
  },
  Limit(u64),
  Skip(u64),
  Project(Value),
}

impl Stage {
  pub fn to_value(&self) -> Value {
    match self {
      Stage::Match(filter) => serde_json::json!({ "$match": filter }),
      Stage::Group { id, accumulators } => {
        let mut obj = serde_json::Map::new();
        obj.insert("_id".to_string(), id.clone());
        for (k, v) in accumulators {
          obj.insert(k.clone(), v.clone());
        }
        serde_json::json!({ "$group": Value::Object(obj) })
      }
      Stage::Sort { field, ascending } => {
        serde_json::json!({ "$sort": { field: if *ascending { 1 } else { -1 } } })
      }
      Stage::Limit(n) => serde_json::json!({ "$limit": n }),
      Stage::Skip(n) => serde_json::json!({ "$skip": n }),
      Stage::Project(fields) => serde_json::json!({ "$project": fields }),
    }
  }

  pub async fn execute(&self, docs: Vec<Value>) -> OrmResult<Vec<Value>> {
    match self {
      Stage::Match(filter) => Ok(
        docs
          .into_iter()
          .filter(|d| value_matches(&filter, d))
          .collect(),
      ),
      Stage::Group { id, accumulators } => {
        let mut groups: std::collections::HashMap<String, Vec<Value>> =
          std::collections::HashMap::new();

        for doc in docs {
          let key = if id.is_null() {
            "null".to_string()
          } else if let Some(s) = id.as_str() {
            doc.get(s).map(|v| v.to_string()).unwrap_or_default()
          } else {
            serde_json::to_string(&doc).unwrap_or_default()
          };
          groups.entry(key).or_default().push(doc);
        }

        let mut results = Vec::new();
        for (key, group_docs) in groups {
          let mut result = serde_json::Map::new();
          result.insert("_id".to_string(), serde_json::json!(key));

          for (acc_name, acc_expr) in accumulators {
            let value = Self::compute_accumulator(acc_expr, &group_docs);
            result.insert(acc_name.clone(), value);
          }

          results.push(Value::Object(result));
        }

        Ok(results)
      }
      Stage::Sort { field, ascending } => {
        let mut sorted = docs;
        sorted.sort_by(|a, b| {
          let cmp = a
            .get(field)
            .map(|v| v.to_string())
            .unwrap_or_default()
            .cmp(&b.get(field).map(|v| v.to_string()).unwrap_or_default());
          if *ascending {
            cmp
          } else {
            cmp.reverse()
          }
        });
        Ok(sorted)
      }
      Stage::Limit(n) => Ok(docs.into_iter().take(*n as usize).collect()),
      Stage::Skip(n) => Ok(docs.into_iter().skip(*n as usize).collect()),
      Stage::Project(fields) => {
        let projected: Vec<Value> = docs
          .into_iter()
          .map(|doc| {
            let mut result = serde_json::Map::new();
            if let Some(obj) = fields.as_object() {
              for (key, value) in obj {
                if let Some(src_field) = value.as_str() {
                  if let Some(v) = doc.get(src_field) {
                    result.insert(key.clone(), v.clone());
                  }
                } else if value.as_object().is_some() {
                  result.insert(key.clone(), doc.clone());
                }
              }
            }
            Value::Object(result)
          })
          .collect();
        Ok(projected)
      }
    }
  }

  fn compute_accumulator(expr: &Value, docs: &[Value]) -> Value {
    if let Some(obj) = expr.as_object() {
      if let Some(sum) = obj.get("$sum") {
        let field = sum.as_str().unwrap_or("_id");
        let total: f64 = docs
          .iter()
          .filter_map(|d| d.get(field).and_then(|v| v.as_f64()))
          .sum();
        return serde_json::json!(total);
      }
      if let Some(avg) = obj.get("$avg") {
        let field = avg.as_str().unwrap_or("_id");
        let values: Vec<f64> = docs
          .iter()
          .filter_map(|d| d.get(field).and_then(|v| v.as_f64()))
          .collect();
        let avg_val = if values.is_empty() {
          0.0
        } else {
          values.iter().sum::<f64>() / values.len() as f64
        };
        return serde_json::json!(avg_val);
      }
      if let Some(min) = obj.get("$min") {
        let field = min.as_str().unwrap_or("_id");
        let min_val = docs
          .iter()
          .filter_map(|d| d.get(field))
          .min_by(|a, b| a.to_string().cmp(&b.to_string()));
        return min_val.cloned().unwrap_or(serde_json::Value::Null);
      }
      if let Some(max) = obj.get("$max") {
        let field = max.as_str().unwrap_or("_id");
        let max_val = docs
          .iter()
          .filter_map(|d| d.get(field))
          .max_by(|a, b| a.to_string().cmp(&b.to_string()));
        return max_val.cloned().unwrap_or(serde_json::Value::Null);
      }
      if let Some(_count) = obj.get("$count") {
        return serde_json::json!(docs.len());
      }
    }
    serde_json::Value::Null
  }
}

pub trait Aggregation {
  fn to_pipeline(&self) -> AggregationPipeline;
}

fn value_matches(filter: &Value, doc: &Value) -> bool {
  if let Some(obj) = filter.as_object() {
    for (key, val) in obj {
      if let Some(doc_val) = doc.get(key) {
        if doc_val != val {
          return false;
        }
      } else {
        return false;
      }
    }
    true
  } else {
    true
  }
}
