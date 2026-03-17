use fax_types::*;
use fax_anp::*;
use serde::{Deserialize, Serialize};

use crate::config::FaxConfig;
use crate::tools::FaxToolRunner;
use crate::audit::FaxAuditLog;

/// A complete FAX-enabled OpenFang agent that wires together
/// identity, ANP communication, trading protocol, and blockchain anchoring.
pub struct FaxAgent {
    pub identity: AgentIdentity,
    pub config: FaxConfig,
    pub tool_runner: FaxToolRunner,
    pub audit_log: FaxAuditLog,
    pub session: Option<FaxSession>,
}

impl FaxAgent {
    /// Create a new FAX agent from identity and config.
    pub fn new(identity: AgentIdentity, config: FaxConfig) -> Self {
        let capabilities = config.capabilities.to_capabilities();
        let chain_config = config.to_chain_config();

        Self {
            tool_runner: FaxToolRunner::new(&identity.did, capabilities, chain_config),
            identity,
            config,
            audit_log: FaxAuditLog::new(),
            session: None,
        }
    }

    /// Generate a fresh agent with FAX capabilities.
    pub fn generate(domain: &str, name: &str, config: FaxConfig) -> FaxResult<Self> {
        let identity = AgentIdentity::generate(domain, name)?;
        Ok(Self::new(identity, config))
    }

    /// Build the ANP Agent Description for this agent.
    pub fn agent_description(&self) -> FaxAgentDescription {
        let domain = self.identity.did
            .strip_prefix("did:wba:")
            .unwrap_or("localhost")
            .split(':')
            .next()
            .unwrap_or("localhost");

        let mut builder = AgentDescriptionBuilder::new(
            &self.identity.did,
            &self.identity.display_name,
            domain,
        )
        .description("FAX-enabled trading agent");

        builder = builder.security_level(SecurityLevel::Escrow);
        builder.build()
    }

    /// Build the DID Document for this agent.
    pub fn did_document(&self) -> DidDocument {
        let domain = self.identity.did
            .strip_prefix("did:wba:")
            .unwrap_or("localhost")
            .split(':')
            .next()
            .unwrap_or("localhost");

        build_did_document(&self.identity, domain)
    }

    /// Build the A2A Agent Card for this agent.
    /// This includes FAX tools as skills that other agents can invoke.
    pub fn agent_card(&self) -> AgentCard {
        let tools = crate::tools::fax_tool_definitions();
        let skills: Vec<AgentSkill> = tools.iter().map(|t| AgentSkill {
            id: t.name.clone(),
            name: t.name.replace('_', " "),
            description: t.description.clone(),
            input_schema: t.input_schema.clone(),
        }).collect();

        AgentCard {
            name: self.identity.display_name.clone(),
            description: "FAX resource trading agent".into(),
            did: self.identity.did.clone(),
            version: "1.0.0".into(),
            capabilities: vec!["fax-trading".into(), "resource-exchange".into()],
            skills,
            default_input_modes: vec!["application/json".into()],
            default_output_modes: vec!["application/json".into()],
        }
    }

    /// Open a trading session with a peer agent over ANP.
    pub fn open_session(&mut self, peer_did: &str) -> &mut FaxSession {
        self.session = Some(FaxSession::new(&self.identity.did, peer_did));
        self.session.as_mut().unwrap()
    }

    /// Execute an FAX tool call (wired from OpenFang's tool_runner).
    pub async fn execute_tool(&mut self, tool_name: &str, input: serde_json::Value) -> crate::tools::ToolResult {
        let result = self.tool_runner.execute(tool_name, input).await;

        let action = match tool_name {
            "fax_discover" => crate::audit::FaxAuditAction::DiscoverySearch,
            "fax_create_offer" => crate::audit::FaxAuditAction::ResourceOffer,
            "fax_accept_offer" => crate::audit::FaxAuditAction::ResourceAccept,
            "fax_lock_resource" => crate::audit::FaxAuditAction::ResourceLock,
            "fax_deliver" => crate::audit::FaxAuditAction::ResourceDeliver,
            "fax_anchor" => crate::audit::FaxAuditAction::ChainAnchor,
            "fax_check_reputation" => crate::audit::FaxAuditAction::ReputationQuery,
            _ => crate::audit::FaxAuditAction::DiscoverySearch,
        };
        let outcome = if result.success { "ok" } else { "error" };
        self.audit_log.record(&self.identity.did, action, None, &result.output, outcome);

        result
    }

    /// Get the agent's EVM address for on-chain operations.
    pub fn evm_address(&self) -> String {
        self.identity.evm_address.clone().unwrap_or_default()
    }
}

/// A2A Agent Card structure matching OpenFang's format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub did: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub skills: Vec<AgentSkill>,
    pub default_input_modes: Vec<String>,
    pub default_output_modes: Vec<String>,
}

/// A skill entry in the Agent Card — one per FAX tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::FaxManifestCapabilities;

    fn test_config() -> FaxConfig {
        FaxConfig {
            capabilities: FaxManifestCapabilities {
                fax_offer: vec!["*".into()],
                fax_accept: true,
                fax_discover: true,
                fax_anchor: true,
                fax_chain_sign: true,
                fax_escrow_max_rcu: Some(1000.0),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_agent_creation() {
        let agent = FaxAgent::generate("test.com", "alpha", test_config()).unwrap();
        assert!(agent.identity.did.contains("alpha"));
        assert!(!agent.evm_address().is_empty());
    }

    #[test]
    fn test_agent_description() {
        let agent = FaxAgent::generate("test.com", "alpha", test_config()).unwrap();
        let ad = agent.agent_description();
        assert_eq!(ad.protocol_type, "ANP");
        assert!(ad.interfaces.iter().any(|i| i.protocol == "FAX"));
    }

    #[test]
    fn test_did_document() {
        let agent = FaxAgent::generate("test.com", "alpha", test_config()).unwrap();
        let doc = agent.did_document();
        assert!(doc.id.starts_with("did:wba:"));
        let services = doc.service.unwrap();
        assert!(services.iter().any(|s| s.service_type == "FaxTradingEndpoint"));
    }

    #[test]
    fn test_agent_card() {
        let agent = FaxAgent::generate("test.com", "alpha", test_config()).unwrap();
        let card = agent.agent_card();
        assert!(!card.skills.is_empty());
        assert!(card.capabilities.contains(&"fax-trading".to_string()));
        assert!(card.skills.iter().any(|s| s.id == "fax_create_offer"));
    }

    #[tokio::test]
    async fn test_agent_execute_tool_with_audit() {
        let mut agent = FaxAgent::generate("test.com", "alpha", test_config()).unwrap();
        let result = agent.execute_tool("fax_rates", serde_json::json!({})).await;
        assert!(result.success);
        assert_eq!(agent.audit_log.len(), 1);
        assert!(agent.audit_log.verify_integrity().is_ok());
    }

    #[tokio::test]
    async fn test_agent_trade_with_audit_chain() {
        let mut agent = FaxAgent::generate("test.com", "trader", test_config()).unwrap();

        agent.execute_tool("fax_discover", serde_json::json!({"needs_resource": "compute"})).await;
        agent.execute_tool("fax_create_offer", serde_json::json!({
            "counterparty_did": "did:wba:other.com:user:bob",
            "offer_type": "compute",
            "offer_amount": 2.0,
            "offer_unit": "gpu-hour",
            "request_type": "llm_tokens",
            "request_amount": 100000.0,
            "request_unit": "tokens"
        })).await;
        agent.execute_tool("fax_check_reputation", serde_json::json!({"agent_address": "0xBob"})).await;

        assert_eq!(agent.audit_log.len(), 3);
        assert!(agent.audit_log.verify_integrity().is_ok());
    }
}
