use fax_types::*;
use serde::{Deserialize, Serialize};

/// A negotiation message exchanged during the meta-protocol phase.
/// Agents negotiate trade terms, security level, and exchange rates before committing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationMessage {
    pub trade_id: String,
    pub from_did: String,
    pub to_did: String,
    pub action: NegotiationAction,
    pub security_proposal: SecurityProposal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NegotiationAction {
    /// Initial proposal with resources and indicative rates.
    Propose {
        offered: Vec<ResourceAmount>,
        requested: Vec<ResourceAmount>,
        rcu_value: f64,
        message: Option<String>,
    },
    /// Counter-offer with modified terms.
    Counter {
        offered: Vec<ResourceAmount>,
        requested: Vec<ResourceAmount>,
        rcu_value: f64,
        message: Option<String>,
    },
    /// Accept the current terms.
    Accept {
        agreed_rcu_value: f64,
    },
    /// Reject the trade entirely.
    Reject {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityProposal {
    pub proposed_level: SecurityLevel,
    pub acceptable_levels: Vec<SecurityLevel>,
    pub my_reputation_score: Option<u64>,
    pub require_blockchain_anchor: bool,
}

impl SecurityProposal {
    pub fn new(level: SecurityLevel) -> Self {
        Self {
            proposed_level: level,
            acceptable_levels: vec![level],
            my_reputation_score: None,
            require_blockchain_anchor: level >= SecurityLevel::Anchor,
        }
    }

    pub fn with_acceptable(mut self, levels: Vec<SecurityLevel>) -> Self {
        self.acceptable_levels = levels;
        self
    }

    pub fn with_reputation(mut self, score: u64) -> Self {
        self.my_reputation_score = Some(score);
        self
    }
}

/// Determines the appropriate security level for a trade based on context.
pub fn recommend_security_level(
    rcu_value: f64,
    counterparty_reputation: Option<u64>,
    is_first_trade: bool,
) -> SecurityLevel {
    let rep_score = counterparty_reputation.unwrap_or(0);

    if rcu_value > 1000.0 || rep_score < 300 {
        return SecurityLevel::FullEscrow;
    }
    if rcu_value > 100.0 || is_first_trade || rep_score < 600 {
        return SecurityLevel::Escrow;
    }
    if rcu_value > 10.0 {
        return SecurityLevel::Anchor;
    }
    SecurityLevel::Trust
}

/// Check if two security proposals are compatible (have overlapping acceptable levels).
pub fn negotiate_security_level(
    proposal_a: &SecurityProposal,
    proposal_b: &SecurityProposal,
) -> Option<SecurityLevel> {
    let mut best: Option<SecurityLevel> = None;
    for level in &proposal_a.acceptable_levels {
        if proposal_b.acceptable_levels.contains(level) {
            match best {
                None => best = Some(*level),
                Some(current) if *level > current => best = Some(*level),
                _ => {}
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_recommendation() {
        assert_eq!(
            recommend_security_level(5.0, Some(900), false),
            SecurityLevel::Trust
        );
        assert_eq!(
            recommend_security_level(50.0, Some(800), false),
            SecurityLevel::Anchor
        );
        assert_eq!(
            recommend_security_level(50.0, None, true),
            SecurityLevel::FullEscrow
        );
        assert_eq!(
            recommend_security_level(2000.0, Some(900), false),
            SecurityLevel::FullEscrow
        );
    }

    #[test]
    fn test_negotiate_compatible() {
        let a = SecurityProposal::new(SecurityLevel::Escrow)
            .with_acceptable(vec![SecurityLevel::Anchor, SecurityLevel::Escrow, SecurityLevel::FullEscrow]);
        let b = SecurityProposal::new(SecurityLevel::Anchor)
            .with_acceptable(vec![SecurityLevel::Anchor, SecurityLevel::Escrow]);

        let agreed = negotiate_security_level(&a, &b);
        assert_eq!(agreed, Some(SecurityLevel::Escrow));
    }

    #[test]
    fn test_negotiate_incompatible() {
        let a = SecurityProposal::new(SecurityLevel::Trust)
            .with_acceptable(vec![SecurityLevel::Trust]);
        let b = SecurityProposal::new(SecurityLevel::FullEscrow)
            .with_acceptable(vec![SecurityLevel::FullEscrow]);

        assert_eq!(negotiate_security_level(&a, &b), None);
    }
}
