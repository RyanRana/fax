use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::identity::sha256_hex;
use crate::resource::ResourceAmount;
use crate::error::{FaxError, FaxResult};

/// The type of a verifiable credential in the FAX trade lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialType {
    ResourceOffer,
    ResourceCounterOffer,
    SwapAgreement,
    ResourceLock,
    ResourceDelivery,
    SwapCompletion,
    AnchorReceipt,
    DisputeInitiation,
    DisputeResolution,
}

/// A verifiable credential in the FAX protocol.
/// Each credential is linked to the previous one via `previous_credential_hash`,
/// forming a tamper-evident hash chain anchored on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaxCredential {
    pub id: String,
    pub credential_type: CredentialType,
    pub issuer_did: String,
    pub subject: CredentialSubject,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub previous_credential_hash: Option<String>,
    pub proof: Option<CredentialProof>,
}

/// The subject/payload of a credential, varying by type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CredentialSubject {
    ResourceOffer {
        trade_id: String,
        offered: Vec<ResourceAmount>,
        requested: Vec<ResourceAmount>,
        rcu_value: f64,
        expiry: DateTime<Utc>,
    },
    ResourceCounterOffer {
        trade_id: String,
        original_offer_id: String,
        counter_offered: Vec<ResourceAmount>,
        counter_requested: Vec<ResourceAmount>,
        rcu_value: f64,
        expiry: DateTime<Utc>,
    },
    SwapAgreement {
        trade_id: String,
        party_a_did: String,
        party_b_did: String,
        party_a_gives: Vec<ResourceAmount>,
        party_b_gives: Vec<ResourceAmount>,
        rcu_value: f64,
        security_level: u8,
        lock_duration_secs: u64,
    },
    ResourceLock {
        trade_id: String,
        locker_did: String,
        hash_lock: String,
        resource_endpoint: String,
        lock_expiry: DateTime<Utc>,
    },
    ResourceDelivery {
        trade_id: String,
        deliverer_did: String,
        secret_reveal: String,
        delivery_proof_hash: String,
    },
    SwapCompletion {
        trade_id: String,
        party_a_did: String,
        party_b_did: String,
        completed_at: DateTime<Utc>,
    },
    AnchorReceipt {
        trade_id: String,
        chain_hash: String,
        tx_hash: String,
        block_number: u64,
        anchored_at: DateTime<Utc>,
    },
    DisputeInitiation {
        trade_id: String,
        initiator_did: String,
        reason: String,
        evidence_hash: String,
    },
    DisputeResolution {
        trade_id: String,
        arbitrator_did: String,
        decision: String,
        favor_party: String,
    },
}

/// Cryptographic proof attached to a credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialProof {
    pub proof_type: String,
    pub verification_method: String,
    pub signature: String,
    pub created: DateTime<Utc>,
}

impl FaxCredential {
    pub fn new(
        credential_type: CredentialType,
        issuer_did: String,
        subject: CredentialSubject,
    ) -> Self {
        Self {
            id: format!("urn:uuid:{}", Uuid::new_v4()),
            credential_type,
            issuer_did,
            subject,
            issued_at: Utc::now(),
            expires_at: None,
            previous_credential_hash: None,
            proof: None,
        }
    }

    /// Link this credential to the previous one in the chain.
    pub fn chain_after(mut self, previous: &FaxCredential) -> Self {
        self.previous_credential_hash = Some(previous.compute_hash());
        self
    }

    /// Compute the SHA-256 hash of this credential's canonical JSON representation.
    pub fn compute_hash(&self) -> String {
        let json = serde_json::to_string(self).expect("credential serializable");
        sha256_hex(json.as_bytes())
    }

    /// Set the expiry time.
    pub fn with_expiry(mut self, expiry: DateTime<Utc>) -> Self {
        self.expires_at = Some(expiry);
        self
    }

    /// Attach a proof (signature) to this credential.
    pub fn with_proof(mut self, proof: CredentialProof) -> Self {
        self.proof = Some(proof);
        self
    }

    /// Get the trade_id from the credential subject.
    pub fn trade_id(&self) -> &str {
        match &self.subject {
            CredentialSubject::ResourceOffer { trade_id, .. }
            | CredentialSubject::ResourceCounterOffer { trade_id, .. }
            | CredentialSubject::SwapAgreement { trade_id, .. }
            | CredentialSubject::ResourceLock { trade_id, .. }
            | CredentialSubject::ResourceDelivery { trade_id, .. }
            | CredentialSubject::SwapCompletion { trade_id, .. }
            | CredentialSubject::AnchorReceipt { trade_id, .. }
            | CredentialSubject::DisputeInitiation { trade_id, .. }
            | CredentialSubject::DisputeResolution { trade_id, .. } => trade_id,
        }
    }
}

/// An ordered chain of FAX credentials forming a trade's complete history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialChain {
    pub trade_id: String,
    pub credentials: Vec<FaxCredential>,
}

impl CredentialChain {
    pub fn new(trade_id: impl Into<String>) -> Self {
        Self {
            trade_id: trade_id.into(),
            credentials: Vec::new(),
        }
    }

    /// Append a credential to the chain, automatically linking it.
    pub fn append(&mut self, mut credential: FaxCredential) {
        if let Some(last) = self.credentials.last() {
            credential.previous_credential_hash = Some(last.compute_hash());
        }
        self.credentials.push(credential);
    }

    /// Verify the integrity of the entire chain.
    /// Each credential's `previous_credential_hash` must match the hash of the prior credential.
    pub fn verify_integrity(&self) -> FaxResult<()> {
        for i in 1..self.credentials.len() {
            let expected_hash = self.credentials[i - 1].compute_hash();
            let recorded_hash = self.credentials[i]
                .previous_credential_hash
                .as_ref()
                .ok_or_else(|| FaxError::BrokenChain {
                    index: i,
                    reason: "missing previous_credential_hash".into(),
                })?;

            if *recorded_hash != expected_hash {
                return Err(FaxError::BrokenChain {
                    index: i,
                    reason: format!(
                        "hash mismatch: recorded {}, computed {}",
                        recorded_hash, expected_hash
                    ),
                });
            }
        }
        Ok(())
    }

    /// Get the hash of the chain tip (latest credential).
    pub fn tip_hash(&self) -> Option<String> {
        self.credentials.last().map(|c| c.compute_hash())
    }

    pub fn len(&self) -> usize {
        self.credentials.len()
    }

    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{ResourceAmount, ResourceType};

    fn make_offer(trade_id: &str) -> FaxCredential {
        FaxCredential::new(
            CredentialType::ResourceOffer,
            "did:wba:example.com:user:alice".into(),
            CredentialSubject::ResourceOffer {
                trade_id: trade_id.into(),
                offered: vec![ResourceAmount::new(ResourceType::Compute, 2.0, "gpu-hour")],
                requested: vec![ResourceAmount::new(ResourceType::LlmTokens, 100000.0, "tokens")],
                rcu_value: 100.0,
                expiry: Utc::now() + chrono::Duration::hours(1),
            },
        )
    }

    fn make_agreement(trade_id: &str) -> FaxCredential {
        FaxCredential::new(
            CredentialType::SwapAgreement,
            "did:wba:example.com:user:bob".into(),
            CredentialSubject::SwapAgreement {
                trade_id: trade_id.into(),
                party_a_did: "did:wba:example.com:user:alice".into(),
                party_b_did: "did:wba:example.com:user:bob".into(),
                party_a_gives: vec![ResourceAmount::new(ResourceType::Compute, 2.0, "gpu-hour")],
                party_b_gives: vec![ResourceAmount::new(ResourceType::LlmTokens, 100000.0, "tokens")],
                rcu_value: 100.0,
                security_level: 2,
                lock_duration_secs: 3600,
            },
        )
    }

    #[test]
    fn test_credential_chain_integrity() {
        let mut chain = CredentialChain::new("trade-001");
        chain.append(make_offer("trade-001"));
        chain.append(make_agreement("trade-001"));
        assert!(chain.verify_integrity().is_ok());
    }

    #[test]
    fn test_chain_detects_tampering() {
        let mut chain = CredentialChain::new("trade-001");
        chain.append(make_offer("trade-001"));
        chain.append(make_agreement("trade-001"));

        // Tamper with the first credential
        if let CredentialSubject::ResourceOffer { rcu_value, .. } = &mut chain.credentials[0].subject {
            *rcu_value = 999.0;
        }
        assert!(chain.verify_integrity().is_err());
    }

    #[test]
    fn test_tip_hash_changes_with_chain() {
        let mut chain = CredentialChain::new("trade-001");
        chain.append(make_offer("trade-001"));
        let h1 = chain.tip_hash().unwrap();
        chain.append(make_agreement("trade-001"));
        let h2 = chain.tip_hash().unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_credential_hash_deterministic() {
        let c1 = make_offer("trade-001");
        let h1 = c1.compute_hash();
        let h2 = c1.compute_hash();
        assert_eq!(h1, h2);
    }
}
