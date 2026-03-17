use fax_types::*;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// DID Document structure following the DID:WBA spec (§3).
/// Extended with FAX service endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidDocument {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    pub id: String,
    #[serde(rename = "verificationMethod")]
    pub verification_method: Vec<VerificationMethod>,
    pub authentication: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "keyAgreement")]
    pub key_agreement: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<Vec<DidService>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationMethod {
    pub id: String,
    #[serde(rename = "type")]
    pub method_type: String,
    pub controller: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "publicKeyMultibase")]
    pub public_key_multibase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "publicKeyJwk")]
    pub public_key_jwk: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidService {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    pub service_endpoint: String,
}

/// Build a DID Document from an FAX AgentIdentity with proper ANP-compatible structure.
pub fn build_did_document(identity: &AgentIdentity, domain: &str) -> DidDocument {
    let did = &identity.did;
    let _pubkey_hex = identity.public_key_hex();
    let pubkey_multibase = format!("z{}", bs58_encode(&identity.verifying_key_bytes));

    let ed25519_vm = VerificationMethod {
        id: format!("{did}#key-ed25519-1"),
        method_type: "Ed25519VerificationKey2020".into(),
        controller: did.clone(),
        public_key_multibase: Some(pubkey_multibase),
        public_key_jwk: None,
    };

    let ad_url = format!("https://{domain}/agents/{}/ad.json", identity.display_name);
    let fax_url = format!("https://{domain}/agents/{}/fax-interface.json", identity.display_name);

    DidDocument {
        context: vec![
            "https://www.w3.org/ns/did/v1".into(),
            "https://w3id.org/security/suites/ed25519-2020/v1".into(),
        ],
        id: did.clone(),
        verification_method: vec![ed25519_vm],
        authentication: vec![serde_json::json!(format!("{did}#key-ed25519-1"))],
        key_agreement: None,
        service: Some(vec![
            DidService {
                id: format!("{did}#agent-description"),
                service_type: "AgentDescription".into(),
                service_endpoint: ad_url,
            },
            DidService {
                id: format!("{did}#fax-trading"),
                service_type: "FaxTradingEndpoint".into(),
                service_endpoint: fax_url,
            },
        ]),
    }
}

/// Derive the well-known DID document URL from a DID:WBA identifier.
/// did:wba:example.com:user:alice → https://example.com/user/alice/did.json
pub fn did_to_url(did: &str) -> Option<String> {
    let parts: Vec<&str> = did.strip_prefix("did:wba:")?.splitn(2, ':').collect();
    if parts.len() < 2 {
        return None;
    }
    let domain = parts[0];
    let path = parts[1].replace(':', "/");
    Some(format!("https://{domain}/{path}/did.json"))
}

/// Derive a deterministic EVM address from a DID's Ed25519 public key.
/// Used for on-chain operations (anchoring, escrow, reputation).
pub fn did_to_evm_address(public_key_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"fax-evm-derivation:");
    hasher.update(public_key_bytes);
    let hash = hasher.finalize();
    format!("0x{}", hex::encode(&hash[12..32]))
}

/// Minimal base58 encoding (Bitcoin alphabet) for multibase keys.
fn bs58_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    if data.is_empty() {
        return String::new();
    }

    let mut digits: Vec<u8> = vec![0];
    for &byte in data {
        let mut carry = byte as u32;
        for digit in digits.iter_mut() {
            carry += (*digit as u32) * 256;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }

    let leading_zeros = data.iter().take_while(|&&b| b == 0).count();
    let mut result = String::with_capacity(leading_zeros + digits.len());
    for _ in 0..leading_zeros {
        result.push('1');
    }
    for &d in digits.iter().rev() {
        result.push(ALPHABET[d as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_did_to_url() {
        assert_eq!(
            did_to_url("did:wba:example.com:user:alice"),
            Some("https://example.com/user/alice/did.json".into())
        );
        assert_eq!(
            did_to_url("did:wba:compute.io:agents:alpha"),
            Some("https://compute.io/agents/alpha/did.json".into())
        );
    }

    #[test]
    fn test_build_did_document() {
        let identity = AgentIdentity::generate("example.com", "testbot").unwrap();
        let doc = build_did_document(&identity, "example.com");

        assert!(doc.id.starts_with("did:wba:"));
        assert!(!doc.verification_method.is_empty());
        assert_eq!(doc.verification_method[0].method_type, "Ed25519VerificationKey2020");
        assert!(doc.service.is_some());
        let services = doc.service.unwrap();
        assert_eq!(services.len(), 2);
        assert!(services.iter().any(|s| s.service_type == "FaxTradingEndpoint"));
    }

    #[test]
    fn test_evm_address_derivation() {
        let identity = AgentIdentity::generate("x.com", "test").unwrap();
        let addr = did_to_evm_address(&identity.verifying_key_bytes);
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42); // 0x + 40 hex chars
    }
}
