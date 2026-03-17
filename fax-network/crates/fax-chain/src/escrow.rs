use fax_types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Simulated on-chain escrow state for development.
/// In production, this reads from the FAXEscrow smart contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowState {
    pub trade_id: String,
    pub party_a: String,
    pub party_b: String,
    pub hash_lock_a: String,
    pub hash_lock_b: String,
    pub rcu_value: f64,
    pub state: EscrowTradeState,
    pub lock_expiry: u64,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscrowTradeState {
    None,
    Locked,
    ADelivered,
    BDelivered,
    Complete,
    Expired,
    Disputed,
    Resolved,
}

/// Simulated escrow service for development/testing.
pub struct EscrowService {
    trades: HashMap<String, EscrowState>,
}

impl EscrowService {
    pub fn new() -> Self {
        Self {
            trades: HashMap::new(),
        }
    }

    pub fn lock_trade(
        &mut self,
        trade_id: &str,
        party_a: &str,
        party_b: &str,
        hash_lock_a: &str,
        hash_lock_b: &str,
        rcu_value: f64,
        lock_duration_secs: u64,
    ) -> FaxResult<()> {
        if self.trades.contains_key(trade_id) {
            return Err(FaxError::Other("trade already exists in escrow".into()));
        }

        let now = chrono::Utc::now().timestamp() as u64;
        self.trades.insert(trade_id.to_string(), EscrowState {
            trade_id: trade_id.to_string(),
            party_a: party_a.to_string(),
            party_b: party_b.to_string(),
            hash_lock_a: hash_lock_a.to_string(),
            hash_lock_b: hash_lock_b.to_string(),
            rcu_value,
            state: EscrowTradeState::Locked,
            lock_expiry: now + lock_duration_secs,
            created_at: now,
        });

        tracing::info!(trade_id, party_a, party_b, rcu_value, "trade locked in escrow");
        Ok(())
    }

    pub fn confirm_delivery(&mut self, trade_id: &str, party: &str, secret: &str) -> FaxResult<()> {
        let trade = self.trades.get_mut(trade_id)
            .ok_or_else(|| FaxError::Other("trade not found".into()))?;

        let is_a = party == trade.party_a;
        let is_b = party == trade.party_b;
        if !is_a && !is_b {
            return Err(FaxError::Other("not a party to this trade".into()));
        }

        let secret_bytes = hex::decode(secret)
            .map_err(|_| FaxError::Other("invalid secret hex".into()))?;
        let expected_lock = if is_a { &trade.hash_lock_a } else { &trade.hash_lock_b };
        let expected_bytes = hex::decode(expected_lock)
            .map_err(|_| FaxError::Other("invalid hash-lock hex".into()))?;

        if !fax_protocol::HashLockSecret::verify(&secret_bytes, &expected_bytes) {
            return Err(FaxError::HashLockMismatch);
        }

        trade.state = match (&trade.state, is_a) {
            (EscrowTradeState::Locked, true) => EscrowTradeState::ADelivered,
            (EscrowTradeState::Locked, false) => EscrowTradeState::BDelivered,
            (EscrowTradeState::ADelivered, false) => EscrowTradeState::Complete,
            (EscrowTradeState::BDelivered, true) => EscrowTradeState::Complete,
            _ => return Err(FaxError::InvalidState {
                expected: "Locked or partially delivered".into(),
                actual: format!("{:?}", trade.state),
            }),
        };

        Ok(())
    }

    pub fn get_trade(&self, trade_id: &str) -> Option<&EscrowState> {
        self.trades.get(trade_id)
    }

    pub fn init_dispute(&mut self, trade_id: &str, party: &str) -> FaxResult<()> {
        let trade = self.trades.get_mut(trade_id)
            .ok_or_else(|| FaxError::Other("trade not found".into()))?;
        if party != trade.party_a && party != trade.party_b {
            return Err(FaxError::Other("not a party".into()));
        }
        trade.state = EscrowTradeState::Disputed;
        Ok(())
    }
}

impl Default for EscrowService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fax_protocol::HashLockSecret;

    #[test]
    fn test_escrow_full_lifecycle() {
        let mut escrow = EscrowService::new();
        let secret_a = HashLockSecret::generate();
        let secret_b = HashLockSecret::generate();

        escrow.lock_trade(
            "trade-001", "alice", "bob",
            &secret_a.hash_lock_hex(), &secret_b.hash_lock_hex(),
            100.0, 3600,
        ).unwrap();

        escrow.confirm_delivery("trade-001", "alice", &secret_a.secret_hex()).unwrap();
        assert_eq!(escrow.get_trade("trade-001").unwrap().state, EscrowTradeState::ADelivered);

        escrow.confirm_delivery("trade-001", "bob", &secret_b.secret_hex()).unwrap();
        assert_eq!(escrow.get_trade("trade-001").unwrap().state, EscrowTradeState::Complete);
    }

    #[test]
    fn test_escrow_wrong_secret() {
        let mut escrow = EscrowService::new();
        let secret_a = HashLockSecret::generate();
        let secret_b = HashLockSecret::generate();

        escrow.lock_trade(
            "trade-002", "alice", "bob",
            &secret_a.hash_lock_hex(), &secret_b.hash_lock_hex(),
            50.0, 3600,
        ).unwrap();

        let result = escrow.confirm_delivery("trade-002", "alice", &secret_b.secret_hex());
        assert!(result.is_err());
    }
}
