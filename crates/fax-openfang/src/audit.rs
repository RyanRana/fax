use sha2::{Sha256, Digest};
use serde::{Deserialize, Serialize};

/// FAX audit actions that extend OpenFang's AuditAction enum.
/// These should be added to `openfang-runtime/src/audit.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaxAuditAction {
    ResourceOffer,
    ResourceAccept,
    ResourceLock,
    ResourceDeliver,
    TradeComplete,
    ChainAnchor,
    DisputeInitiate,
    DisputeResolve,
    ReputationQuery,
    DiscoverySearch,
}

impl std::fmt::Display for FaxAuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResourceOffer => write!(f, "fax:resource_offer"),
            Self::ResourceAccept => write!(f, "fax:resource_accept"),
            Self::ResourceLock => write!(f, "fax:resource_lock"),
            Self::ResourceDeliver => write!(f, "fax:resource_deliver"),
            Self::TradeComplete => write!(f, "fax:trade_complete"),
            Self::ChainAnchor => write!(f, "fax:chain_anchor"),
            Self::DisputeInitiate => write!(f, "fax:dispute_initiate"),
            Self::DisputeResolve => write!(f, "fax:dispute_resolve"),
            Self::ReputationQuery => write!(f, "fax:reputation_query"),
            Self::DiscoverySearch => write!(f, "fax:discovery_search"),
        }
    }
}

/// An FAX-specific audit entry that mirrors OpenFang's AuditEntry format.
/// In production, this would be written to the same SQLite audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaxAuditEntry {
    pub seq: u64,
    pub timestamp: String,
    pub agent_id: String,
    pub action: FaxAuditAction,
    pub trade_id: Option<String>,
    pub detail: String,
    pub outcome: String,
    pub prev_hash: String,
    pub hash: String,
}

/// FAX audit log that extends OpenFang's Merkle hash chain.
/// Each entry's hash includes the previous entry's hash,
/// creating a tamper-evident chain that can be anchored on L2.
pub struct FaxAuditLog {
    entries: Vec<FaxAuditEntry>,
    tip_hash: String,
    next_seq: u64,
}

impl FaxAuditLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            tip_hash: "0".repeat(64),
            next_seq: 0,
        }
    }

    /// Record an FAX audit entry, chaining it to the previous one.
    pub fn record(
        &mut self,
        agent_id: &str,
        action: FaxAuditAction,
        trade_id: Option<&str>,
        detail: &str,
        outcome: &str,
    ) -> &FaxAuditEntry {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let prev_hash = self.tip_hash.clone();

        let hash = compute_entry_hash(
            self.next_seq,
            &timestamp,
            agent_id,
            &action.to_string(),
            detail,
            outcome,
            &prev_hash,
        );

        let entry = FaxAuditEntry {
            seq: self.next_seq,
            timestamp,
            agent_id: agent_id.to_string(),
            action,
            trade_id: trade_id.map(|s| s.to_string()),
            detail: detail.to_string(),
            outcome: outcome.to_string(),
            prev_hash,
            hash: hash.clone(),
        };

        self.entries.push(entry);
        self.tip_hash = hash;
        self.next_seq += 1;

        self.entries.last().unwrap()
    }

    /// Verify the entire audit chain is intact.
    pub fn verify_integrity(&self) -> Result<(), String> {
        let mut expected_prev = "0".repeat(64);

        for (i, entry) in self.entries.iter().enumerate() {
            if entry.prev_hash != expected_prev {
                return Err(format!(
                    "chain broken at seq {}: expected prev_hash {}, got {}",
                    i, expected_prev, entry.prev_hash
                ));
            }

            let computed = compute_entry_hash(
                entry.seq,
                &entry.timestamp,
                &entry.agent_id,
                &entry.action.to_string(),
                &entry.detail,
                &entry.outcome,
                &entry.prev_hash,
            );

            if computed != entry.hash {
                return Err(format!(
                    "hash mismatch at seq {}: computed {}, stored {}",
                    i, computed, entry.hash
                ));
            }

            expected_prev = entry.hash.clone();
        }

        Ok(())
    }

    /// Get the tip hash for blockchain anchoring.
    pub fn tip_hash(&self) -> &str {
        &self.tip_hash
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[FaxAuditEntry] {
        &self.entries
    }
}

impl Default for FaxAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

fn compute_entry_hash(
    seq: u64,
    timestamp: &str,
    agent_id: &str,
    action: &str,
    detail: &str,
    outcome: &str,
    prev_hash: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(seq.to_be_bytes());
    hasher.update(timestamp.as_bytes());
    hasher.update(agent_id.as_bytes());
    hasher.update(action.as_bytes());
    hasher.update(detail.as_bytes());
    hasher.update(outcome.as_bytes());
    hasher.update(prev_hash.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_chain_integrity() {
        let mut log = FaxAuditLog::new();

        log.record("agent-alice", FaxAuditAction::ResourceOffer, Some("trade-001"),
            "offer 2 gpu-hours for 100k tokens", "ok");
        log.record("agent-alice", FaxAuditAction::ResourceLock, Some("trade-001"),
            "locked compute at wss://alice/gpu", "ok");
        log.record("agent-alice", FaxAuditAction::ResourceDeliver, Some("trade-001"),
            "revealed secret", "ok");
        log.record("agent-alice", FaxAuditAction::TradeComplete, Some("trade-001"),
            "swap completed", "ok");
        log.record("agent-alice", FaxAuditAction::ChainAnchor, Some("trade-001"),
            "anchored on block #42", "ok");

        assert_eq!(log.len(), 5);
        assert!(log.verify_integrity().is_ok());
    }

    #[test]
    fn test_audit_detects_tampering() {
        let mut log = FaxAuditLog::new();

        log.record("agent-alice", FaxAuditAction::ResourceOffer, Some("trade-001"),
            "offer", "ok");
        log.record("agent-alice", FaxAuditAction::ResourceLock, Some("trade-001"),
            "lock", "ok");

        // Tamper with the first entry
        log.entries.first_mut().unwrap().detail = "tampered".to_string();

        assert!(log.verify_integrity().is_err());
    }

    #[test]
    fn test_audit_tip_hash_changes() {
        let mut log = FaxAuditLog::new();
        let h1 = log.tip_hash().to_string();

        log.record("agent-a", FaxAuditAction::DiscoverySearch, None, "search compute", "ok");
        let h2 = log.tip_hash().to_string();

        assert_ne!(h1, h2);
    }
}
