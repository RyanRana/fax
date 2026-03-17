use fax_types::*;
use serde::{Deserialize, Serialize};

use crate::agent_description::FaxAgentDescription;

/// A discovered FAX-capable agent from ANP's discovery protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAgent {
    pub did: String,
    pub name: String,
    pub ad_url: String,
    pub fax_interface_url: Option<String>,
    pub offered_resource_types: Vec<String>,
    pub accepted_resource_types: Vec<String>,
    pub min_security_level: SecurityLevel,
    pub reputation_score: Option<u64>,
}

/// Query filter for discovering FAX-capable agents.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryQuery {
    pub needs_resource: Option<ResourceType>,
    pub offers_resource: Option<ResourceType>,
    pub min_reputation: Option<u64>,
    pub max_security_level: Option<SecurityLevel>,
}

impl DiscoveryQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn needs(mut self, rtype: ResourceType) -> Self {
        self.needs_resource = Some(rtype);
        self
    }

    pub fn offers(mut self, rtype: ResourceType) -> Self {
        self.offers_resource = Some(rtype);
        self
    }

    pub fn min_reputation(mut self, score: u64) -> Self {
        self.min_reputation = Some(score);
        self
    }

    pub fn max_security(mut self, level: SecurityLevel) -> Self {
        self.max_security_level = Some(level);
        self
    }
}

/// Discovery service that queries ANP's `/.well-known/agent-descriptions`
/// endpoints and filters for FAX-capable agents.
pub struct DiscoveryService {
    known_agents: Vec<DiscoveredAgent>,
    known_domains: Vec<String>,
}

impl DiscoveryService {
    pub fn new() -> Self {
        Self {
            known_agents: Vec::new(),
            known_domains: Vec::new(),
        }
    }

    /// Register a domain to crawl for FAX agents.
    pub fn add_domain(&mut self, domain: impl Into<String>) {
        self.known_domains.push(domain.into());
    }

    /// Register a known agent directly (bypass discovery).
    pub fn register_agent(&mut self, agent: DiscoveredAgent) {
        self.known_agents.push(agent);
    }

    /// Parse an ANP Agent Description and extract FAX-relevant info.
    pub fn parse_agent_description(&self, ad: &FaxAgentDescription) -> Option<DiscoveredAgent> {
        let fax_interface = ad.interfaces.iter()
            .find(|i| i.protocol == "FAX")?;

        let offered_types: Vec<String> = ad.informations.iter()
            .filter_map(|i| i.fax_resource_type.clone())
            .collect();

        Some(DiscoveredAgent {
            did: ad.did.clone(),
            name: ad.name.clone(),
            ad_url: ad.url.clone(),
            fax_interface_url: Some(fax_interface.url.clone()),
            offered_resource_types: offered_types,
            accepted_resource_types: Vec::new(),
            min_security_level: if fax_interface.human_authorization == Some(true) {
                SecurityLevel::FullEscrow
            } else {
                SecurityLevel::Anchor
            },
            reputation_score: None,
        })
    }

    /// Crawl a domain's `/.well-known/agent-descriptions` endpoint.
    /// Returns the well-known URL to fetch. Actual HTTP fetching is done by the caller.
    pub fn well_known_url(domain: &str) -> String {
        format!("https://{}/.well-known/agent-descriptions", domain)
    }

    /// Filter known agents by a discovery query.
    pub fn query(&self, filter: &DiscoveryQuery) -> Vec<&DiscoveredAgent> {
        self.known_agents.iter().filter(|agent| {
            if let Some(ref needs) = filter.needs_resource {
                let type_str = needs.to_string();
                if !agent.offered_resource_types.iter().any(|t| t == &type_str) {
                    return false;
                }
            }
            if let Some(ref offers) = filter.offers_resource {
                let type_str = offers.to_string();
                if !agent.accepted_resource_types.iter().any(|t| t == &type_str) {
                    return false;
                }
            }
            if let Some(min_rep) = filter.min_reputation {
                if agent.reputation_score.unwrap_or(0) < min_rep {
                    return false;
                }
            }
            if let Some(max_sec) = filter.max_security_level {
                if agent.min_security_level > max_sec {
                    return false;
                }
            }
            true
        }).collect()
    }

    pub fn all_agents(&self) -> &[DiscoveredAgent] {
        &self.known_agents
    }

    pub fn agent_count(&self) -> usize {
        self.known_agents.len()
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentDescriptionBuilder;

    fn make_compute_agent() -> DiscoveredAgent {
        DiscoveredAgent {
            did: "did:wba:compute.io:user:alpha".into(),
            name: "Compute Alpha".into(),
            ad_url: "https://compute.io/agents/alpha/ad.json".into(),
            fax_interface_url: Some("https://compute.io/agents/alpha/fax-interface.json".into()),
            offered_resource_types: vec!["compute".into()],
            accepted_resource_types: vec!["llm_tokens".into()],
            min_security_level: SecurityLevel::Anchor,
            reputation_score: Some(850),
        }
    }

    fn make_knowledge_agent() -> DiscoveredAgent {
        DiscoveredAgent {
            did: "did:wba:knowledge.ai:user:beta".into(),
            name: "Knowledge Beta".into(),
            ad_url: "https://knowledge.ai/agents/beta/ad.json".into(),
            fax_interface_url: Some("https://knowledge.ai/agents/beta/fax-interface.json".into()),
            offered_resource_types: vec!["knowledge_access".into(), "llm_tokens".into()],
            accepted_resource_types: vec!["compute".into()],
            min_security_level: SecurityLevel::Escrow,
            reputation_score: Some(720),
        }
    }

    #[test]
    fn test_query_by_resource_type() {
        let mut service = DiscoveryService::new();
        service.register_agent(make_compute_agent());
        service.register_agent(make_knowledge_agent());

        let results = service.query(&DiscoveryQuery::new().needs(ResourceType::Compute));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Compute Alpha");

        let results = service.query(&DiscoveryQuery::new().needs(ResourceType::LlmTokens));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Knowledge Beta");
    }

    #[test]
    fn test_query_by_reputation() {
        let mut service = DiscoveryService::new();
        service.register_agent(make_compute_agent());
        service.register_agent(make_knowledge_agent());

        let results = service.query(&DiscoveryQuery::new().min_reputation(800));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Compute Alpha");
    }

    #[test]
    fn test_parse_from_ad() {
        let ad = AgentDescriptionBuilder::new(
            "did:wba:x.com:user:test", "Test Agent", "x.com"
        )
        .offer_resource(TradableResource {
            resource: ResourceAmount::new(ResourceType::Compute, 4.0, "gpu-hour"),
            quality: None, min_trade: None, max_trade: None, availability_windows: None,
        })
        .build();

        let service = DiscoveryService::new();
        let discovered = service.parse_agent_description(&ad).unwrap();
        assert_eq!(discovered.did, "did:wba:x.com:user:test");
        assert!(discovered.fax_interface_url.is_some());
    }

    #[test]
    fn test_well_known_url() {
        assert_eq!(
            DiscoveryService::well_known_url("compute.io"),
            "https://compute.io/.well-known/agent-descriptions"
        );
    }
}
