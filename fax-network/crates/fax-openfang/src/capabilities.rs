use fax_types::SecurityLevel;
use serde::{Deserialize, Serialize};

/// FAX capabilities that extend OpenFang's capability system.
/// These map to variants that should be added to OpenFang's `Capability` enum
/// in `openfang-types/src/capability.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FaxCapability {
    /// Can offer resources for trade. Pattern matches resource types (e.g., "compute/*").
    FaxOffer(String),

    /// Can accept incoming trade requests.
    FaxAccept,

    /// Can lock resources in escrow (hash-lock or on-chain).
    FaxEscrow { max_rcu: f64 },

    /// Can sign blockchain transactions (requires secp256k1 key derivation).
    FaxChainSign,

    /// Can anchor VC chain hashes on L2.
    FaxAnchor,

    /// Can extend credit (IOU) up to a limit in RCU.
    FaxCredit(f64),

    /// Can participate in dispute resolution as arbitrator.
    FaxArbitrate,

    /// Can discover and connect to other FAX-capable agents.
    FaxDiscover,
}

/// Manifest fields for FAX capabilities in a Hand's HAND.toml.
/// These should be added to OpenFang's `ManifestCapabilities` struct.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FaxManifestCapabilities {
    #[serde(default)]
    pub fax_offer: Vec<String>,
    #[serde(default)]
    pub fax_accept: bool,
    #[serde(default)]
    pub fax_escrow_max_rcu: Option<f64>,
    #[serde(default)]
    pub fax_chain_sign: bool,
    #[serde(default)]
    pub fax_anchor: bool,
    #[serde(default)]
    pub fax_credit_limit: Option<f64>,
    #[serde(default)]
    pub fax_arbitrate: bool,
    #[serde(default)]
    pub fax_discover: bool,
}

impl FaxManifestCapabilities {
    /// Convert manifest capabilities to the runtime capability list.
    pub fn to_capabilities(&self) -> Vec<FaxCapability> {
        let mut caps = Vec::new();

        for pattern in &self.fax_offer {
            caps.push(FaxCapability::FaxOffer(pattern.clone()));
        }
        if self.fax_accept {
            caps.push(FaxCapability::FaxAccept);
        }
        if let Some(max_rcu) = self.fax_escrow_max_rcu {
            caps.push(FaxCapability::FaxEscrow { max_rcu });
        }
        if self.fax_chain_sign {
            caps.push(FaxCapability::FaxChainSign);
        }
        if self.fax_anchor {
            caps.push(FaxCapability::FaxAnchor);
        }
        if let Some(limit) = self.fax_credit_limit {
            caps.push(FaxCapability::FaxCredit(limit));
        }
        if self.fax_arbitrate {
            caps.push(FaxCapability::FaxArbitrate);
        }
        if self.fax_discover {
            caps.push(FaxCapability::FaxDiscover);
        }

        caps
    }
}

/// Check if a requested FAX operation is allowed by the agent's capabilities.
pub fn capability_check(
    caps: &[FaxCapability],
    required: &FaxCapability,
) -> bool {
    caps.iter().any(|cap| match (cap, required) {
        (FaxCapability::FaxOffer(pattern), FaxCapability::FaxOffer(requested)) => {
            pattern == "*" || pattern == requested || requested.starts_with(pattern.trim_end_matches('*'))
        }
        (FaxCapability::FaxAccept, FaxCapability::FaxAccept) => true,
        (FaxCapability::FaxEscrow { max_rcu }, FaxCapability::FaxEscrow { max_rcu: needed }) => {
            max_rcu >= needed
        }
        (FaxCapability::FaxChainSign, FaxCapability::FaxChainSign) => true,
        (FaxCapability::FaxAnchor, FaxCapability::FaxAnchor) => true,
        (FaxCapability::FaxCredit(limit), FaxCapability::FaxCredit(needed)) => {
            limit >= needed
        }
        (FaxCapability::FaxArbitrate, FaxCapability::FaxArbitrate) => true,
        (FaxCapability::FaxDiscover, FaxCapability::FaxDiscover) => true,
        _ => false,
    })
}

/// Determine the minimum security level an agent can support based on their capabilities.
pub fn max_supported_security(caps: &[FaxCapability]) -> SecurityLevel {
    let has_anchor = caps.iter().any(|c| matches!(c, FaxCapability::FaxAnchor));
    let has_escrow = caps.iter().any(|c| matches!(c, FaxCapability::FaxEscrow { .. }));
    let has_chain = caps.iter().any(|c| matches!(c, FaxCapability::FaxChainSign));
    let has_arbitrate = caps.iter().any(|c| matches!(c, FaxCapability::FaxArbitrate));

    if has_escrow && has_chain && has_anchor && has_arbitrate {
        SecurityLevel::ZkPrivate
    } else if has_escrow && has_chain && has_anchor {
        SecurityLevel::FullEscrow
    } else if has_escrow && has_chain {
        SecurityLevel::Escrow
    } else if has_anchor {
        SecurityLevel::Anchor
    } else {
        SecurityLevel::Trust
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_check_offer_pattern() {
        let caps = vec![FaxCapability::FaxOffer("compute/*".into())];
        assert!(capability_check(&caps, &FaxCapability::FaxOffer("compute/gpu".into())));
        assert!(!capability_check(&caps, &FaxCapability::FaxOffer("storage/ssd".into())));

        let wildcard = vec![FaxCapability::FaxOffer("*".into())];
        assert!(capability_check(&wildcard, &FaxCapability::FaxOffer("anything".into())));
    }

    #[test]
    fn test_capability_check_escrow_limit() {
        let caps = vec![FaxCapability::FaxEscrow { max_rcu: 500.0 }];
        assert!(capability_check(&caps, &FaxCapability::FaxEscrow { max_rcu: 200.0 }));
        assert!(!capability_check(&caps, &FaxCapability::FaxEscrow { max_rcu: 1000.0 }));
    }

    #[test]
    fn test_max_security_level() {
        let basic = vec![FaxCapability::FaxOffer("*".into())];
        assert_eq!(max_supported_security(&basic), SecurityLevel::Trust);

        let anchored = vec![FaxCapability::FaxAnchor, FaxCapability::FaxOffer("*".into())];
        assert_eq!(max_supported_security(&anchored), SecurityLevel::Anchor);

        let full = vec![
            FaxCapability::FaxAnchor,
            FaxCapability::FaxEscrow { max_rcu: 1000.0 },
            FaxCapability::FaxChainSign,
        ];
        assert_eq!(max_supported_security(&full), SecurityLevel::FullEscrow);
    }

    #[test]
    fn test_manifest_to_capabilities() {
        let manifest = FaxManifestCapabilities {
            fax_offer: vec!["compute/*".into(), "llm_tokens".into()],
            fax_accept: true,
            fax_escrow_max_rcu: Some(500.0),
            fax_chain_sign: true,
            fax_anchor: true,
            fax_credit_limit: None,
            fax_arbitrate: false,
            fax_discover: true,
        };
        let caps = manifest.to_capabilities();
        assert_eq!(caps.len(), 7);
        assert!(capability_check(&caps, &FaxCapability::FaxOffer("compute/gpu".into())));
        assert!(capability_check(&caps, &FaxCapability::FaxAccept));
        assert!(capability_check(&caps, &FaxCapability::FaxDiscover));
    }
}
