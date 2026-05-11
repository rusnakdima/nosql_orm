use crate::error::{OrmError, OrmResult};
use crate::provider::DatabaseProvider;
use serde_json::Value;
use std::time::Instant;

pub struct EtlPipeline<S, T>
where
    S: DatabaseProvider,
    T: DatabaseProvider,
{
    source: S,
    transformers: Vec<Box<dyn Transformer>>,
    destination: T,
}

impl<S, T> EtlPipeline<S, T>
where
    S: DatabaseProvider,
    T: DatabaseProvider,
{
    pub fn new(source: S, destination: T) -> Self {
        Self {
            source,
            transformers: Vec::new(),
            destination,
        }
    }

    pub fn add_transformer(mut self, transformer: Box<dyn Transformer>) -> Self {
        self.transformers.push(transformer);
        self
    }

    pub async fn run(&self, batch_size: usize, collection: &str) -> OrmResult<EtlStats> {
        let start = Instant::now();
        let mut extracted = 0;
        let mut transformed = 0;
        let mut loaded = 0;
        let mut errors = 0;

        loop {
            let docs = self
                .source
                .find_many(collection, None, None, Some(batch_size as u64), None, true)
                .await?;

            if docs.is_empty() {
                break;
            }

            extracted += docs.len();

            for doc in docs {
                let mut current = doc;
                let mut transformed_ok = true;

                for transformer in &self.transformers {
                    match transformer.transform(current) {
                        Ok(result) => current = result,
                        Err(_) => {
                            transformed_ok = false;
                            errors += 1;
                            break;
                        }
                    }
                }

                if transformed_ok {
                    match self.destination.insert(collection, current).await {
                        Ok(_) => {
                            transformed += 1;
                            loaded += 1;
                        }
                        Err(_) => {
                            errors += 1;
                        }
                    }
                }
            }
        }

        Ok(EtlStats {
            extracted,
            transformed,
            loaded,
            errors,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

pub trait Transformer: Send + Sync {
    fn transform(&self, input: Value) -> OrmResult<Value>;
    fn transform_batch(&self, inputs: Vec<Value>) -> OrmResult<Vec<Value>> {
        inputs.into_iter().map(|i| self.transform(i)).collect()
    }
}

#[derive(Debug, Clone)]
pub struct EtlStats {
    pub extracted: usize,
    pub transformed: usize,
    pub loaded: usize,
    pub errors: usize,
    pub duration_ms: u64,
}

impl Default for EtlStats {
    fn default() -> Self {
        Self {
            extracted: 0,
            transformed: 0,
            loaded: 0,
            errors: 0,
            duration_ms: 0,
        }
    }
}

pub struct IdentityTransformer;

impl Transformer for IdentityTransformer {
    fn transform(&self, input: Value) -> OrmResult<Value> {
        Ok(input)
    }
}

pub struct FieldMapperTransformer {
    field_mappings: Vec<(String, String)>,
}

impl FieldMapperTransformer {
    pub fn new(mappings: Vec<(String, String)>) -> Self {
        Self { field_mappings: mappings }
    }
}

impl Transformer for FieldMapperTransformer {
    fn transform(&self, input: Value) -> OrmResult<Value> {
        let mut map = serde_json::Map::new();
        let obj = input.as_object().ok_or_else(|| OrmError::InvalidInput("Expected object".to_string()))?;

        for (from, to) in &self.field_mappings {
            if let Some(value) = obj.get(from) {
                map.insert(to.clone(), value.clone());
            }
        }

        for (key, value) in obj {
            if !self.field_mappings.iter().any(|(from, _)| from == key) {
                map.insert(key.clone(), value.clone());
            }
        }

        Ok(Value::Object(map))
    }
}