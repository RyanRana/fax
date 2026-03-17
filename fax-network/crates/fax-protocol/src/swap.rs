use fax_types::*;
use chrono::{Duration, Utc};
use rand::RngCore;
use sha2::{Sha256, Digest};

/// A hash-lock secret used in atomic resource swaps.
/// The secret is generated locally and only revealed after the counterparty has locked their resource.
#[derive(Debug, Clone)]
pub struct HashLockSecret {
    pub secret: [u8; 32],
    pub hash_lock: [u8; 32],
}

impl HashLockSecret {
    /// Generate a new random secret and its corresponding hash-lock.
    pub fn generate() -> Self {
        let mut secret = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret);

        let mut hasher = Sha256::new();
        hasher.update(secret);
        let hash_lock: [u8; 32] = hasher.finalize().into();

        Self { secret, hash_lock }
    }

    pub fn secret_hex(&self) -> String {
        hex::encode(self.secret)
    }

    pub fn hash_lock_hex(&self) -> String {
        hex::encode(self.hash_lock)
    }

    /// Verify that a revealed secret matches a hash-lock.
    pub fn verify(secret_bytes: &[u8], expected_hash_lock: &[u8]) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(secret_bytes);
        let computed: [u8; 32] = hasher.finalize().into();
        computed.as_slice() == expected_hash_lock
    }
}

/// The state machine for an atomic hash-lock swap between two agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapState {
    /// Initial state — agreement signed but no locks yet.
    Agreed,
    /// Party A has locked their resource.
    ALocked,
    /// Party B has locked their resource (both locked).
    BothLocked,
    /// Party A has revealed their secret (delivered).
    ADelivered,
    /// Party B has revealed their secret (delivered).
    BDelivered,
    /// Both secrets revealed — swap complete.
    Complete,
    /// Lock expired before completion.
    Expired,
    /// Dispute initiated.
    Disputed,
}

/// Manages the state and credentials for a single atomic swap.
pub struct SwapEngine {
    pub trade_id: String,
    pub state: SwapState,
    pub my_secret: HashLockSecret,
    pub counterparty_hash_lock: Option<[u8; 32]>,
    pub chain: CredentialChain,
    pub lock_duration: Duration,
    my_locked: bool,
    counterparty_locked: bool,
    my_delivered: bool,
    counterparty_delivered: bool,
}

impl SwapEngine {
    pub fn new(trade_id: impl Into<String>, lock_duration_secs: i64) -> Self {
        let trade_id = trade_id.into();
        Self {
            chain: CredentialChain::new(&trade_id),
            trade_id,
            state: SwapState::Agreed,
            my_secret: HashLockSecret::generate(),
            counterparty_hash_lock: None,
            lock_duration: Duration::seconds(lock_duration_secs),
            my_locked: false,
            counterparty_locked: false,
            my_delivered: false,
            counterparty_delivered: false,
        }
    }

    /// Create a ResourceLockCredential for our side of the swap.
    pub fn create_lock_credential(
        &mut self,
        my_did: &str,
        resource_endpoint: &str,
    ) -> FaxResult<FaxCredential> {
        let lock_expiry = Utc::now() + self.lock_duration;

        let credential = FaxCredential::new(
            CredentialType::ResourceLock,
            my_did.to_string(),
            CredentialSubject::ResourceLock {
                trade_id: self.trade_id.clone(),
                locker_did: my_did.to_string(),
                hash_lock: self.my_secret.hash_lock_hex(),
                resource_endpoint: resource_endpoint.to_string(),
                lock_expiry,
            },
        );

        self.my_locked = true;
        self.chain.append(credential.clone());
        self.update_state();
        Ok(credential)
    }

    /// Process a lock credential received from the counterparty.
    pub fn receive_lock(
        &mut self,
        credential: FaxCredential,
    ) -> FaxResult<()> {
        if let CredentialSubject::ResourceLock { hash_lock, .. } = &credential.subject {
            let hash_bytes = hex::decode(hash_lock)
                .map_err(|e| FaxError::Other(format!("invalid hash-lock hex: {e}")))?;
            let mut arr = [0u8; 32];
            if hash_bytes.len() != 32 {
                return Err(FaxError::Other("hash-lock must be 32 bytes".into()));
            }
            arr.copy_from_slice(&hash_bytes);
            self.counterparty_hash_lock = Some(arr);
            self.counterparty_locked = true;
            self.chain.append(credential);
            self.update_state();
            Ok(())
        } else {
            Err(FaxError::Other("expected ResourceLock credential".into()))
        }
    }

    /// Create a ResourceDeliveryCredential by revealing our secret.
    pub fn create_delivery_credential(
        &mut self,
        my_did: &str,
    ) -> FaxResult<FaxCredential> {
        if self.state != SwapState::BothLocked
            && self.state != SwapState::ADelivered
            && self.state != SwapState::BDelivered
        {
            return Err(FaxError::InvalidState {
                expected: "BothLocked, ADelivered, or BDelivered".into(),
                actual: format!("{:?}", self.state),
            });
        }

        let delivery_proof = sha256_hex(self.my_secret.secret_hex().as_bytes());

        let credential = FaxCredential::new(
            CredentialType::ResourceDelivery,
            my_did.to_string(),
            CredentialSubject::ResourceDelivery {
                trade_id: self.trade_id.clone(),
                deliverer_did: my_did.to_string(),
                secret_reveal: self.my_secret.secret_hex(),
                delivery_proof_hash: delivery_proof,
            },
        );

        self.my_delivered = true;
        self.chain.append(credential.clone());
        self.update_state();
        Ok(credential)
    }

    /// Process a delivery credential from the counterparty (verify their secret).
    pub fn receive_delivery(
        &mut self,
        credential: FaxCredential,
    ) -> FaxResult<()> {
        if let CredentialSubject::ResourceDelivery { secret_reveal, .. } = &credential.subject {
            let secret_bytes = hex::decode(secret_reveal)
                .map_err(|e| FaxError::Other(format!("invalid secret hex: {e}")))?;

            let expected_lock = self.counterparty_hash_lock
                .ok_or_else(|| FaxError::Other("no counterparty hash-lock set".into()))?;

            if !HashLockSecret::verify(&secret_bytes, &expected_lock) {
                return Err(FaxError::HashLockMismatch);
            }

            self.counterparty_delivered = true;
            self.chain.append(credential);
            self.update_state();
            Ok(())
        } else {
            Err(FaxError::Other("expected ResourceDelivery credential".into()))
        }
    }

    /// Create a SwapCompletionCredential (dual-signed in practice).
    pub fn create_completion_credential(
        &mut self,
        party_a_did: &str,
        party_b_did: &str,
    ) -> FaxResult<FaxCredential> {
        if self.state != SwapState::Complete {
            return Err(FaxError::InvalidState {
                expected: "Complete".into(),
                actual: format!("{:?}", self.state),
            });
        }

        let credential = FaxCredential::new(
            CredentialType::SwapCompletion,
            party_a_did.to_string(),
            CredentialSubject::SwapCompletion {
                trade_id: self.trade_id.clone(),
                party_a_did: party_a_did.to_string(),
                party_b_did: party_b_did.to_string(),
                completed_at: Utc::now(),
            },
        );

        self.chain.append(credential.clone());
        Ok(credential)
    }

    fn update_state(&mut self) {
        if self.my_delivered && self.counterparty_delivered {
            self.state = SwapState::Complete;
        } else if self.my_delivered {
            self.state = SwapState::ADelivered;
        } else if self.counterparty_delivered {
            self.state = SwapState::BDelivered;
        } else if self.my_locked && self.counterparty_locked {
            self.state = SwapState::BothLocked;
        } else if self.my_locked || self.counterparty_locked {
            self.state = SwapState::ALocked;
        }
    }

    /// Get the chain tip hash for blockchain anchoring.
    pub fn chain_tip_hash(&self) -> Option<String> {
        self.chain.tip_hash()
    }

    /// Verify the entire credential chain is intact.
    pub fn verify_chain(&self) -> FaxResult<()> {
        self.chain.verify_integrity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_lock_generate_and_verify() {
        let secret = HashLockSecret::generate();
        assert!(HashLockSecret::verify(&secret.secret, &secret.hash_lock));
        assert!(!HashLockSecret::verify(b"wrong", &secret.hash_lock));
    }

    #[test]
    fn test_full_swap_lifecycle() {
        let alice_did = "did:wba:example.com:user:alice";
        let bob_did = "did:wba:example.com:user:bob";

        let mut alice_engine = SwapEngine::new("trade-001", 3600);
        let mut bob_engine = SwapEngine::new("trade-001", 3600);

        // Alice creates her lock
        let alice_lock = alice_engine.create_lock_credential(alice_did, "wss://alice/compute").unwrap();

        // Bob receives Alice's lock
        bob_engine.receive_lock(alice_lock).unwrap();

        // Bob creates his lock
        let bob_lock = bob_engine.create_lock_credential(bob_did, "https://bob/knowledge").unwrap();

        // Alice receives Bob's lock
        alice_engine.receive_lock(bob_lock).unwrap();

        assert_eq!(alice_engine.state, SwapState::BothLocked);

        // Alice reveals her secret (delivers compute access)
        let alice_delivery = alice_engine.create_delivery_credential(alice_did).unwrap();

        // Bob verifies Alice's secret and receives her delivery
        bob_engine.receive_delivery(alice_delivery).unwrap();

        // Bob reveals his secret (delivers knowledge access)
        let bob_delivery = bob_engine.create_delivery_credential(bob_did).unwrap();

        // Alice verifies Bob's secret
        alice_engine.receive_delivery(bob_delivery).unwrap();

        assert_eq!(alice_engine.state, SwapState::Complete);

        // Create completion credential
        let completion = alice_engine.create_completion_credential(alice_did, bob_did).unwrap();
        assert_eq!(completion.credential_type, CredentialType::SwapCompletion);

        // Verify chain integrity
        alice_engine.verify_chain().unwrap();

        // Chain tip hash is available for blockchain anchoring
        assert!(alice_engine.chain_tip_hash().is_some());
    }
}
