use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent reputation data as stored on-chain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OnChainReputation {
    pub total_trades: u64,
    pub successful_trades: u64,
    pub disputes_initiated: u64,
    pub disputes_lost: u64,
    pub total_rcu_traded: f64,
    pub last_trade_block: u64,
    pub registered_block: u64,
}

impl OnChainReputation {
    /// Compute reliability score from 0 to 1000 (mirrors the Solidity contract).
    pub fn reliability_score(&self, current_block: u64) -> u64 {
        if self.total_trades == 0 {
            return 100; // new agent base score
        }

        let completion_score = (self.successful_trades * 700) / self.total_trades;

        let dispute_score = if self.disputes_lost == 0 {
            200
        } else {
            let penalty = (self.disputes_lost * 200) / self.total_trades;
            if penalty >= 200 { 0 } else { 200 - penalty }
        };

        let age = current_block.saturating_sub(self.registered_block);
        let longevity_score = if age >= 2_000_000 { 100 } else { (age * 100) / 2_000_000 };

        completion_score + dispute_score + longevity_score
    }
}

/// Simulated reputation registry for development/testing.
pub struct ReputationService {
    agents: HashMap<String, OnChainReputation>,
    current_block: u64,
}

impl ReputationService {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            current_block: 1,
        }
    }

    pub fn register(&mut self, agent_address: &str) {
        self.agents.entry(agent_address.to_string()).or_insert_with(|| {
            OnChainReputation {
                registered_block: self.current_block,
                ..Default::default()
            }
        });
    }

    pub fn record_completion(
        &mut self,
        party_a: &str,
        party_b: &str,
        rcu_value: f64,
        disputed: bool,
    ) {
        for party in [party_a, party_b] {
            let rep = self.agents.entry(party.to_string()).or_insert_with(|| {
                OnChainReputation {
                    registered_block: self.current_block,
                    ..Default::default()
                }
            });
            rep.total_trades += 1;
            if !disputed {
                rep.successful_trades += 1;
            } else {
                rep.disputes_initiated += 1;
            }
            rep.total_rcu_traded += rcu_value;
            rep.last_trade_block = self.current_block;
        }
        self.current_block += 1;
    }

    pub fn record_dispute_loss(&mut self, agent: &str) {
        if let Some(rep) = self.agents.get_mut(agent) {
            rep.disputes_lost += 1;
        }
    }

    pub fn get_reputation(&self, agent: &str) -> Option<&OnChainReputation> {
        self.agents.get(agent)
    }

    pub fn get_score(&self, agent: &str) -> u64 {
        self.agents.get(agent)
            .map(|r| r.reliability_score(self.current_block))
            .unwrap_or(0)
    }

    pub fn is_registered(&self, agent: &str) -> bool {
        self.agents.contains_key(agent)
    }

    pub fn advance_blocks(&mut self, n: u64) {
        self.current_block += n;
    }
}

impl Default for ReputationService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_agent_score() {
        let mut service = ReputationService::new();
        service.register("0xAlice");
        assert_eq!(service.get_score("0xAlice"), 100);
    }

    #[test]
    fn test_perfect_trader() {
        let mut service = ReputationService::new();
        for _ in 0..20 {
            service.record_completion("0xAlice", "0xBob", 50.0, false);
        }
        let score = service.get_score("0xAlice");
        assert!(score >= 900, "perfect trader should score 900+: {score}");
    }

    #[test]
    fn test_dispute_penalty() {
        let mut service = ReputationService::new();
        for _ in 0..8 {
            service.record_completion("0xAlice", "0xBob", 50.0, false);
        }
        service.record_completion("0xAlice", "0xBob", 50.0, true);
        service.record_completion("0xAlice", "0xBob", 50.0, true);
        service.record_dispute_loss("0xAlice");

        let score = service.get_score("0xAlice");
        let perfect = {
            let mut s = ReputationService::new();
            for _ in 0..10 { s.record_completion("0xP", "0xQ", 50.0, false); }
            s.get_score("0xP")
        };
        assert!(score < perfect, "disputed trader should score lower");
    }

    #[test]
    fn test_longevity_bonus() {
        let mut service = ReputationService::new();
        service.register("0xOld");
        service.record_completion("0xOld", "0xX", 50.0, false);
        let young_score = service.get_score("0xOld");

        service.advance_blocks(2_000_000);
        let old_score = service.get_score("0xOld");
        assert!(old_score > young_score, "longevity should increase score");
    }
}
