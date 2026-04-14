use serde_json::Value;

pub trait AccumulatorFn: Send + Sync {
  fn accumulate(&self, values: &[Value]) -> Value;
  fn name(&self) -> &'static str;
}

pub struct SumAccumulator;

impl AccumulatorFn for SumAccumulator {
  fn accumulate(&self, values: &[Value]) -> Value {
    let sum: f64 = values.iter().filter_map(|v| v.as_f64()).sum();
    serde_json::json!(sum)
  }

  fn name(&self) -> &'static str {
    "$sum"
  }
}

pub struct CountAccumulator;

impl AccumulatorFn for CountAccumulator {
  fn accumulate(&self, values: &[Value]) -> Value {
    serde_json::json!(values.len())
  }

  fn name(&self) -> &'static str {
    "$count"
  }
}

pub struct AvgAccumulator;

impl AccumulatorFn for AvgAccumulator {
  fn accumulate(&self, values: &[Value]) -> Value {
    let nums: Vec<f64> = values.iter().filter_map(|v| v.as_f64()).collect();
    let avg = if nums.is_empty() {
      0.0
    } else {
      nums.iter().sum::<f64>() / nums.len() as f64
    };
    serde_json::json!(avg)
  }

  fn name(&self) -> &'static str {
    "$avg"
  }
}

pub struct Accumulators;

impl Accumulators {
  pub fn get(name: &str) -> Option<Box<dyn AccumulatorFn>> {
    match name {
      "$sum" | "sum" => Some(Box::new(SumAccumulator)),
      "$count" | "count" => Some(Box::new(CountAccumulator)),
      "$avg" | "avg" => Some(Box::new(AvgAccumulator)),
      _ => None,
    }
  }
}

pub trait Accumulator: Send + Sync {
  fn compute(&self, docs: &[Value]) -> Value;
}
