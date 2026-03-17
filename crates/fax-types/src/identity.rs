use ed25519_dalek::{Signer, SigningKey, VerifyingKey, Signature, Verifier};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

use crate::FaxResult;
use crate::error::FaxError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub did: String,
    pub display_name: String,
    #[serde(skip)]
    signing_key: Option<Vec<u8>>,
    pub verifying_key_bytes: Vec<u8>,
    pub evm_address: Option<String>,
}

impl AgentIdentity {
    /// Generate a new agent identity with a fresh Ed25519 keypair.
    /// The DID is derived as did:wba:{domain}:user:{name}.
    pub fn generate(domain: &str, name: &str) -> FaxResult<Self> {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let did = format!("did:wba:{domain}:user:{name}");

        let evm_address = derive_evm_address_from_ed25519(&verifying_key);

        Ok(Self {
            did,
            display_name: name.to_string(),
            signing_key: Some(signing_key.to_bytes().to_vec()),
            verifying_key_bytes: verifying_key.to_bytes().to_vec(),
            evm_address: Some(evm_address),
        })
    }

    /// Sign arbitrary bytes with this agent's Ed25519 key.
    pub fn sign(&self, message: &[u8]) -> FaxResult<Vec<u8>> {
        let key_bytes = self.signing_key.as_ref()
            .ok_or_else(|| FaxError::IdentityError("no signing key loaded".into()))?;
        let bytes: [u8; 32] = key_bytes.as_slice().try_into()
            .map_err(|_| FaxError::IdentityError("invalid key length".into()))?;
        let signing_key = SigningKey::from_bytes(&bytes);
        let signature = signing_key.sign(message);
        Ok(signature.to_bytes().to_vec())
    }

    /// Verify a signature against this agent's public key.
    pub fn verify(&self, message: &[u8], signature_bytes: &[u8]) -> FaxResult<bool> {
        let vk_bytes: [u8; 32] = self.verifying_key_bytes.as_slice().try_into()
            .map_err(|_| FaxError::IdentityError("invalid verifying key length".into()))?;
        let verifying_key = VerifyingKey::from_bytes(&vk_bytes)
            .map_err(|e| FaxError::SignatureError(e.to_string()))?;
        let sig_bytes: [u8; 64] = signature_bytes.try_into()
            .map_err(|_| FaxError::SignatureError("invalid signature length".into()))?;
        let signature = Signature::from_bytes(&sig_bytes);
        Ok(verifying_key.verify(message, &signature).is_ok())
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(&self.verifying_key_bytes)
    }
}

/// Derive a pseudo-EVM address from an Ed25519 public key.
/// In production, agents would have a separate secp256k1 key for EVM.
/// This provides a deterministic mapping for development.
fn derive_evm_address_from_ed25519(key: &VerifyingKey) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"fax-evm-derivation:");
    hasher.update(key.as_bytes());
    let hash = hasher.finalize();
    format!("0x{}", hex::encode(&hash[12..32]))
}

/// Compute SHA-256 hash and return as hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Compute SHA-256 hash and return as raw bytes.
pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identity() {
        let id = AgentIdentity::generate("example.com", "alpha").unwrap();
        assert!(id.did.starts_with("did:wba:example.com:user:alpha"));
        assert!(!id.verifying_key_bytes.is_empty());
        assert!(id.evm_address.is_some());
    }

    #[test]
    fn test_sign_and_verify() {
        let id = AgentIdentity::generate("example.com", "signer").unwrap();
        let message = b"hello fax";
        let sig = id.sign(message).unwrap();
        assert!(id.verify(message, &sig).unwrap());
        assert!(!id.verify(b"wrong message", &sig).unwrap());
    }

    #[test]
    fn test_evm_address_deterministic() {
        let id1 = AgentIdentity::generate("a.com", "test").unwrap();
        // Different agents get different addresses
        let id2 = AgentIdentity::generate("a.com", "test2").unwrap();
        assert_ne!(id1.evm_address, id2.evm_address);
    }
}
