use crate::constraints::IndexType;
use std::collections::HashMap;

#[derive(Debug)]
pub struct IndexRecommendation {
  pub collection: String,
  pub fields: Vec<String>,
  pub index_type: IndexType,
  pub estimated_improvement: f64,
  pub rationale: String,
}

#[derive(Debug)]
pub struct QueryStats {
  pub collection: String,
  pub filter_fields: Vec<String>,
  pub sort_fields: Vec<String>,
  pub frequency: usize,
  pub avg_duration_ms: f64,
}

pub struct IndexRecommender {
  query_history: Vec<QueryStats>,
  min_query_count_for_recommendation: usize,
}

impl IndexRecommender {
  pub fn new() -> Self {
    Self {
      query_history: Vec::new(),
      min_query_count_for_recommendation: 10,
    }
  }

  pub fn record_query(&mut self, stats: QueryStats) {
    self.query_history.push(stats);
  }

  pub fn recommend(&self, collection: &str) -> Vec<IndexRecommendation> {
    let mut recommendations = Vec::new();

    let patterns = self
      .query_history
      .iter()
      .filter(|q| q.collection == collection)
      .collect::<Vec<_>>();

    if patterns.len() < self.min_query_count_for_recommendation {
      return recommendations;
    }

    let mut field_counts: HashMap<Vec<String>, usize> = HashMap::new();
    for pattern in &patterns {
      let mut fields = pattern.filter_fields.clone();
      fields.extend(pattern.sort_fields.clone());
      fields.sort();
      *field_counts.entry(fields).or_insert(0) += pattern.frequency;
    }

    for (fields, count) in field_counts {
      if count >= 10 {
        recommendations.push(IndexRecommendation {
          collection: collection.to_string(),
          fields: fields.clone(),
          index_type: IndexType::BTree,
          estimated_improvement: count as f64 * 0.1,
          rationale: format!("Field combination used in {} queries", count),
        });
      }
    }

    recommendations.sort_by(|a, b| {
      b.estimated_improvement
        .partial_cmp(&a.estimated_improvement)
        .unwrap()
    });
    recommendations
  }
}

impl Default for IndexRecommender {
  fn default() -> Self {
    Self::new()
  }
}
