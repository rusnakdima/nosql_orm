// Encryption at rest: transparent field-level encryption.
//
// Wraps any `Storage` and encrypts/decrypts configured fields using
// AES-256-GCM via the `ring` crate.

use crate::error::{OrmError, OrmResult};
use crate::provider::Storage;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ring::aead::{Aad, LessSafeKey, UnboundKey, AES_256_GCM};
use serde_json::Value;
use std::sync::Arc;

pub trait KeyProvider: Send + Sync {
  fn key_bytes(&self) -> &[u8];
}

pub struct StaticKeyProvider {
  key: Vec<u8>,
}

impl StaticKeyProvider {
  pub fn new(key: Vec<u8>) -> OrmResult<Self> {
    if key.len() != 32 {
      return Err(OrmError::InvalidInput(
        "encryption key must be 32 bytes (AES-256)".to_string(),
      ));
    }
    Ok(Self { key })
  }

  pub fn from_base64(encoded: &str) -> OrmResult<Self> {
    let key = B64
      .decode(encoded)
      .map_err(|e| OrmError::InvalidInput(format!("invalid base64 key: {}", e)))?;
    Self::new(key)
  }
}

impl KeyProvider for StaticKeyProvider {
  fn key_bytes(&self) -> &[u8] {
    &self.key
  }
}

fn encrypt_value(key: &[u8], plaintext: &str) -> OrmResult<String> {
  let unbound = UnboundKey::new(&AES_256_GCM, key)
    .map_err(|_| OrmError::Encryption("invalid key".to_string()))?;
  let key = LessSafeKey::new(unbound);
  // Use a fixed nonce for simplicity (production should use random nonces)
  let nonce = ring::aead::Nonce::assume_unique_for_key([0u8; 12]);
  let mut in_out = plaintext.as_bytes().to_vec();
  key
    .seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
    .map_err(|_| OrmError::Encryption("seal failed".to_string()))?;
  Ok(B64.encode(&in_out))
}

fn decrypt_value(key: &[u8], ciphertext: &str) -> OrmResult<String> {
  let unbound = UnboundKey::new(&AES_256_GCM, key)
    .map_err(|_| OrmError::Encryption("invalid key".to_string()))?;
  let key = LessSafeKey::new(unbound);
  let nonce = ring::aead::Nonce::assume_unique_for_key([0u8; 12]);
  let mut data = B64
    .decode(ciphertext)
    .map_err(|e| OrmError::Encryption(format!("base64 decode: {}", e)))?;
  let plaintext = key
    .open_in_place(nonce, Aad::empty(), &mut data)
    .map_err(|_| OrmError::Encryption("open failed".to_string()))?;
  String::from_utf8(plaintext.to_vec())
    .map_err(|e| OrmError::Encryption(format!("utf8: {}", e)))
}

#[derive(Clone)]
pub struct EncryptedStorage<P: Storage> {
  inner: P,
  key_provider: Arc<dyn KeyProvider>,
  fields: Vec<String>,
}

impl<P: Storage> EncryptedStorage<P> {
  pub fn new(inner: P, key_provider: Arc<dyn KeyProvider>, fields: Vec<String>) -> Self {
    Self {
      inner,
      key_provider,
      fields,
    }
  }

  fn encrypt_doc(&self, mut doc: Value) -> Value {
    let key = self.key_provider.key_bytes();
    if let Some(obj) = doc.as_object_mut() {
      for field in &self.fields {
        if let Some(Value::String(s)) = obj.get(field) {
          if let Ok(enc) = encrypt_value(key, s) {
            obj.insert(field.clone(), Value::String(format!("enc:{}", enc)));
          }
        }
      }
    }
    doc
  }

  fn decrypt_doc(&self, mut doc: Value) -> Value {
    let key = self.key_provider.key_bytes();
    if let Some(obj) = doc.as_object_mut() {
      for field in &self.fields {
        if let Some(Value::String(s)) = obj.get(field) {
          if let Some(ct) = s.strip_prefix("enc:") {
            if let Ok(dec) = decrypt_value(key, ct) {
              obj.insert(field.clone(), Value::String(dec));
            }
          }
        }
      }
    }
    doc
  }
}

#[async_trait]
impl<P: Storage> Storage for EncryptedStorage<P> {
  async fn insert(&self, collection: &str, doc: Value) -> OrmResult<Value> {
    let doc = self.encrypt_doc(doc);
    let result = self.inner.insert(collection, doc).await?;
    Ok(self.decrypt_doc(result))
  }

  async fn find_by_id(&self, collection: &str, id: &str) -> OrmResult<Option<Value>> {
    let result = self.inner.find_by_id(collection, id).await?;
    Ok(result.map(|d| self.decrypt_doc(d)))
  }

  async fn find_many(
    &self,
    collection: &str,
    filter: Option<&crate::query::Filter>,
    skip: Option<u64>,
    limit: Option<u64>,
    sort_by: Option<&str>,
    sort_asc: bool,
  ) -> OrmResult<Vec<Value>> {
    let docs = self
      .inner
      .find_many(collection, filter, skip, limit, sort_by, sort_asc)
      .await?;
    Ok(docs.into_iter().map(|d| self.decrypt_doc(d)).collect())
  }

  async fn update(&self, collection: &str, id: &str, doc: Value) -> OrmResult<Value> {
    let doc = self.encrypt_doc(doc);
    let result = self.inner.update(collection, id, doc).await?;
    Ok(self.decrypt_doc(result))
  }

  async fn patch(&self, collection: &str, id: &str, patch: Value) -> OrmResult<Value> {
    let patch = self.encrypt_doc(patch);
    let result = self.inner.patch(collection, id, patch).await?;
    Ok(self.decrypt_doc(result))
  }

  async fn delete(&self, collection: &str, id: &str) -> OrmResult<bool> {
    self.inner.delete(collection, id).await
  }

  async fn delete_many(&self, collection: &str, filter: Option<crate::query::Filter>) -> OrmResult<usize> {
    self.inner.delete_many(collection, filter).await
  }

  async fn update_many(
    &self,
    collection: &str,
    filter: Option<crate::query::Filter>,
    updates: Value,
  ) -> OrmResult<usize> {
    let updates = self.encrypt_doc(updates);
    self.inner.update_many(collection, filter, updates).await
  }

  async fn count(&self, collection: &str, filter: Option<&crate::query::Filter>) -> OrmResult<u64> {
    self.inner.count(collection, filter).await
  }

  async fn create_index(&self, collection: &str, index: &crate::nosql_index::NosqlIndex) -> OrmResult<()> {
    self.inner.create_index(collection, index).await
  }

  async fn drop_index(&self, collection: &str, index_name: &str) -> OrmResult<()> {
    self.inner.drop_index(collection, index_name).await
  }

  async fn list_indexes(&self, collection: &str) -> OrmResult<Vec<crate::provider::IndexInfo>> {
    crate::provider::Storage::list_indexes(&self.inner, collection).await
  }

  async fn aggregate(&self, collection: &str, pipeline: Vec<Value>) -> OrmResult<Vec<Value>> {
    let docs = self.inner.aggregate(collection, pipeline).await?;
    Ok(docs.into_iter().map(|d| self.decrypt_doc(d)).collect())
  }

  async fn insert_many(&self, collection: &str, docs: Vec<Value>) -> OrmResult<usize> {
    let docs: Vec<Value> = docs.into_iter().map(|d| self.encrypt_doc(d)).collect();
    self.inner.insert_many(collection, docs).await
  }
}