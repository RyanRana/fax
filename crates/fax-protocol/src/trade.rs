use fax_types::*;
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

use crate::swap::SwapEngine;

/// The high-level state of a trade from an agent's perspective.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradePhase {
    Discovery,
    Negotiation,
    Agreement,
    Locking,
    Exchange,
    Confirming,
    Anchoring,
    Complete,
    Expired,
    Disputed,
}

/// A complete trade managed by an agent.
pub struct Trade {
    pub id: String,
    pub phase: TradePhase,
    pub my_did: String,
    pub counterparty_did: Option<String>,
    pub my_offer: Vec<ResourceAmount>,
    pub their_offer: Vec<ResourceAmount>,
    pub rcu_value: f64,
    pub security_level: SecurityLevel,
    pub swap: Option<SwapEngine>,
    pub anchor_tx: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

impl Trade {
    pub fn new(my_did: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            phase: TradePhase::Discovery,
            my_did: my_did.into(),
            counterparty_did: None,
            my_offer: Vec::new(),
            their_offer: Vec::new(),
            rcu_value: 0.0,
            security_level: SecurityLevel::Anchor,
            swap: None,
            anchor_tx: None,
            created_at: Utc::now(),
        }
    }

    /// Create an offer credential to send to a potential trading partner.
    pub fn create_offer(
        &mut self,
        offered: Vec<ResourceAmount>,
        requested: Vec<ResourceAmount>,
    ) -> FaxResult<FaxCredential> {
        self.my_offer = offered.clone();
        self.their_offer = requested.clone();

        let _balance = RcuOracle::trade_balance(&offered, &requested)?;
        let total_rcu: f64 = offered.iter()
            .map(|r| RcuOracle::to_rcu(r).unwrap_or(0.0))
            .sum();
        self.rcu_value = total_rcu;

        let credential = FaxCredential::new(
            CredentialType::ResourceOffer,
            self.my_did.clone(),
            CredentialSubject::ResourceOffer {
                trade_id: self.id.clone(),
                offered,
                requested,
                rcu_value: total_rcu,
                expiry: Utc::now() + chrono::Duration::hours(24),
            },
        );

        self.phase = TradePhase::Negotiation;
        Ok(credential)
    }

    /// Accept a trade by creating a SwapAgreement and initializing the swap engine.
    pub fn accept_and_agree(
        &mut self,
        counterparty_did: impl Into<String>,
        my_gives: Vec<ResourceAmount>,
        they_give: Vec<ResourceAmount>,
        security_level: SecurityLevel,
        lock_duration_secs: i64,
    ) -> FaxResult<FaxCredential> {
        let counterparty = counterparty_did.into();
        self.counterparty_did = Some(counterparty.clone());
        self.my_offer = my_gives.clone();
        self.their_offer = they_give.clone();
        self.security_level = security_level;

        let total_rcu: f64 = my_gives.iter()
            .map(|r| RcuOracle::to_rcu(r).unwrap_or(0.0))
            .sum();
        self.rcu_value = total_rcu;

        let credential = FaxCredential::new(
            CredentialType::SwapAgreement,
            self.my_did.clone(),
            CredentialSubject::SwapAgreement {
                trade_id: self.id.clone(),
                party_a_did: self.my_did.clone(),
                party_b_did: counterparty,
                party_a_gives: my_gives,
                party_b_gives: they_give,
                rcu_value: total_rcu,
                security_level: security_level as u8,
                lock_duration_secs: lock_duration_secs as u64,
            },
        );

        self.swap = Some(SwapEngine::new(&self.id, lock_duration_secs));
        self.phase = TradePhase::Agreement;
        Ok(credential)
    }

    /// Advance to locking phase.
    pub fn begin_locking(&mut self, resource_endpoint: &str) -> FaxResult<FaxCredential> {
        let swap = self.swap.as_mut()
            .ok_or_else(|| FaxError::Other("no swap engine initialized".into()))?;
        let cred = swap.create_lock_credential(&self.my_did, resource_endpoint)?;
        self.phase = TradePhase::Locking;
        Ok(cred)
    }

    /// Process a received lock credential.
    pub fn receive_lock(&mut self, credential: FaxCredential) -> FaxResult<()> {
        let swap = self.swap.as_mut()
            .ok_or_else(|| FaxError::Other("no swap engine initialized".into()))?;
        swap.receive_lock(credential)?;
        if swap.state == crate::swap::SwapState::BothLocked {
            self.phase = TradePhase::Exchange;
        }
        Ok(())
    }

    /// Deliver our resource by revealing the hash-lock secret.
    pub fn deliver(&mut self) -> FaxResult<FaxCredential> {
        let swap = self.swap.as_mut()
            .ok_or_else(|| FaxError::Other("no swap engine initialized".into()))?;
        let cred = swap.create_delivery_credential(&self.my_did)?;
        Ok(cred)
    }

    /// Process the counterparty's delivery (verify their secret).
    pub fn receive_delivery(&mut self, credential: FaxCredential) -> FaxResult<()> {
        let swap = self.swap.as_mut()
            .ok_or_else(|| FaxError::Other("no swap engine initialized".into()))?;
        swap.receive_delivery(credential)?;
        if swap.state == crate::swap::SwapState::Complete {
            self.phase = TradePhase::Confirming;
        }
        Ok(())
    }

    /// Finalize the trade and get the chain tip hash for anchoring.
    pub fn finalize(&mut self) -> FaxResult<(FaxCredential, String)> {
        let counterparty = self.counterparty_did.clone()
            .ok_or_else(|| FaxError::Other("no counterparty".into()))?;
        let swap = self.swap.as_mut()
            .ok_or_else(|| FaxError::Other("no swap engine".into()))?;

        let completion = swap.create_completion_credential(&self.my_did, &counterparty)?;
        let tip_hash = swap.chain_tip_hash()
            .ok_or_else(|| FaxError::Other("empty chain".into()))?;

        self.phase = TradePhase::Anchoring;
        Ok((completion, tip_hash))
    }

    /// Mark the trade as anchored on-chain.
    pub fn set_anchored(&mut self, tx_hash: String) {
        self.anchor_tx = Some(tx_hash);
        self.phase = TradePhase::Complete;
    }

    /// Verify the entire credential chain for this trade.
    pub fn verify_chain(&self) -> FaxResult<()> {
        self.swap.as_ref()
            .ok_or_else(|| FaxError::Other("no swap engine".into()))?
            .verify_chain()
    }
}

/// Manages multiple concurrent trades for an agent.
pub struct TradeManager {
    pub agent_did: String,
    pub active_trades: HashMap<String, Trade>,
    pub completed_trades: Vec<String>,
}

impl TradeManager {
    pub fn new(agent_did: impl Into<String>) -> Self {
        Self {
            agent_did: agent_did.into(),
            active_trades: HashMap::new(),
            completed_trades: Vec::new(),
        }
    }

    pub fn create_trade(&mut self) -> &mut Trade {
        let trade = Trade::new(&self.agent_did);
        let id = trade.id.clone();
        self.active_trades.insert(id.clone(), trade);
        self.active_trades.get_mut(&id).unwrap()
    }

    pub fn get_trade(&self, trade_id: &str) -> Option<&Trade> {
        self.active_trades.get(trade_id)
    }

    pub fn get_trade_mut(&mut self, trade_id: &str) -> Option<&mut Trade> {
        self.active_trades.get_mut(trade_id)
    }

    pub fn complete_trade(&mut self, trade_id: &str) {
        if self.active_trades.remove(trade_id).is_some() {
            self.completed_trades.push(trade_id.to_string());
        }
    }

    pub fn active_count(&self) -> usize {
        self.active_trades.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_trade_lifecycle() {
        let mut alice_mgr = TradeManager::new("did:wba:a.com:user:alice");
        let mut bob_mgr = TradeManager::new("did:wba:b.com:user:bob");

        // Alice creates a trade and an offer
        let alice_trade = alice_mgr.create_trade();
        let offer = alice_trade.create_offer(
            vec![ResourceAmount::new(ResourceType::Compute, 2.0, "gpu-hour").with_subtype("gpu-hour")],
            vec![ResourceAmount::new(ResourceType::LlmTokens, 100000.0, "tokens")],
        ).unwrap();
        let trade_id = alice_trade.id.clone();

        // Bob accepts
        let bob_trade = bob_mgr.create_trade();
        let _agreement = bob_trade.accept_and_agree(
            "did:wba:a.com:user:alice",
            vec![ResourceAmount::new(ResourceType::LlmTokens, 100000.0, "tokens")],
            vec![ResourceAmount::new(ResourceType::Compute, 2.0, "gpu-hour").with_subtype("gpu-hour")],
            SecurityLevel::Escrow,
            3600,
        ).unwrap();

        // Both lock
        let alice_trade = alice_mgr.get_trade_mut(&trade_id).unwrap();
        alice_trade.counterparty_did = Some("did:wba:b.com:user:bob".into());
        alice_trade.swap = Some(SwapEngine::new(&trade_id, 3600));
        let alice_lock = alice_trade.begin_locking("wss://alice/compute").unwrap();

        let bob_trade_id = bob_trade.id.clone();
        let bob_trade = bob_mgr.get_trade_mut(&bob_trade_id).unwrap();
        bob_trade.receive_lock(alice_lock).unwrap();
        let bob_lock = bob_trade.begin_locking("https://bob/tokens").unwrap();

        let alice_trade = alice_mgr.get_trade_mut(&trade_id).unwrap();
        alice_trade.receive_lock(bob_lock).unwrap();

        // Both deliver
        let alice_delivery = alice_trade.deliver().unwrap();
        let bob_trade = bob_mgr.get_trade_mut(&bob_trade_id).unwrap();
        bob_trade.receive_delivery(alice_delivery).unwrap();
        let bob_delivery = bob_trade.deliver().unwrap();
        let alice_trade = alice_mgr.get_trade_mut(&trade_id).unwrap();
        alice_trade.receive_delivery(bob_delivery).unwrap();

        // Finalize
        let (completion, tip_hash) = alice_trade.finalize().unwrap();
        assert!(!tip_hash.is_empty());
        alice_trade.set_anchored("0xfake_tx_hash".into());
        assert_eq!(alice_trade.phase, TradePhase::Complete);
    }
}
