use base64::{engine::general_purpose, Engine};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};

use crate::error::{OrmError, OrmResult};

pub struct FieldEncryption {
    key: LessSafeKey,
}

impl FieldEncryption {
    pub fn new(key_base64: &str) -> OrmResult<Self> {
        let key_bytes = general_purpose::STANDARD
            .decode(key_base64)
            .map_err(|e| OrmError::Security(format!("Base64 decode error: {}", e)))?;
        if key_bytes.len() != 32 {
            return Err(OrmError::Security("Encryption key must be 32 bytes".to_string()));
        }
        let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
            .map_err(|_| OrmError::Security("Invalid encryption key".to_string()))?;
        Ok(Self {
            key: LessSafeKey::new(unbound_key),
        })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> OrmResult<Vec<u8>> {
        let rng = SystemRandom::new();
        let mut nonce_bytes = [0u8; 12];
        rng.fill(&mut nonce_bytes)
            .map_err(|_| OrmError::Security("Failed to generate nonce".to_string()))?;

        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut in_out = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
            .map_err(|_| OrmError::Security("Encryption failed".to_string()))?;

        let mut result = nonce_bytes.to_vec();
        result.extend(in_out);
        Ok(result)
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> OrmResult<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(OrmError::Security("Ciphertext too short".to_string()));
        }

        let nonce = Nonce::assume_unique_for_key(
            ciphertext[..12]
                .try_into()
                .map_err(|_| OrmError::Security("Invalid nonce".to_string()))?,
        );
        let mut in_out = ciphertext[12..].to_vec();
        let plaintext = self
            .key
            .open_in_place(nonce, Aad::empty(), &mut in_out)
            .map_err(|_| OrmError::Security("Decryption failed".to_string()))?;

        Ok(plaintext.to_vec())
    }
}

pub trait Encryptable {
    fn encrypt_fields(
        &self,
        encryption: &FieldEncryption,
        fields: &[&str],
    ) -> OrmResult<Self>
    where
        Self: Sized;

    fn decrypt_fields(
        &self,
        encryption: &FieldEncryption,
        fields: &[&str],
    ) -> OrmResult<Self>
    where
        Self: Sized;
}