use fax_types::*;
use fax_protocol::*;
use fax_chain::*;
use fax_anp::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capabilities::{FaxCapability, capability_check};

/// Tool definitions for OpenFang's `builtin_tool_definitions()`.
/// Each tool has a name, description, and JSON Schema for its input.
pub fn fax_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "fax_discover".into(),
            description: "Discover FAX-capable agents that offer or accept specific resource types".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "needs_resource": { "type": "string", "description": "Resource type you need (e.g. 'compute', 'llm_tokens')" },
                    "offers_resource": { "type": "string", "description": "Resource type you're offering" },
                    "min_reputation": { "type": "integer", "description": "Minimum reputation score (0-1000)" },
                    "domain": { "type": "string", "description": "Specific domain to search, or empty for all known domains" }
                }
            }),
        },
        ToolDefinition {
            name: "fax_create_offer".into(),
            description: "Create a resource trade offer to send to another agent".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["counterparty_did", "offer_type", "offer_amount", "offer_unit", "request_type", "request_amount", "request_unit"],
                "properties": {
                    "counterparty_did": { "type": "string", "description": "DID of the agent to trade with" },
                    "offer_type": { "type": "string", "description": "Resource type to offer (e.g. 'compute', 'llm_tokens')" },
                    "offer_amount": { "type": "number", "description": "Amount to offer" },
                    "offer_unit": { "type": "string", "description": "Unit of the offered resource" },
                    "offer_subtype": { "type": "string", "description": "Subtype (e.g. 'gpu-hour')" },
                    "request_type": { "type": "string", "description": "Resource type to request" },
                    "request_amount": { "type": "number", "description": "Amount to request" },
                    "request_unit": { "type": "string", "description": "Unit of the requested resource" }
                }
            }),
        },
        ToolDefinition {
            name: "fax_accept_offer".into(),
            description: "Accept a trade offer and begin the swap process".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["trade_id", "security_level"],
                "properties": {
                    "trade_id": { "type": "string", "description": "ID of the trade to accept" },
                    "security_level": { "type": "integer", "description": "Security level (0=Trust, 1=Anchor, 2=Escrow, 3=FullEscrow)" },
                    "lock_duration_secs": { "type": "integer", "description": "Lock duration in seconds (default 3600)" }
                }
            }),
        },
        ToolDefinition {
            name: "fax_lock_resource".into(),
            description: "Lock your resource behind a hash-lock for an active trade".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["trade_id", "resource_endpoint"],
                "properties": {
                    "trade_id": { "type": "string", "description": "ID of the active trade" },
                    "resource_endpoint": { "type": "string", "description": "URL/endpoint where the counterparty can access the resource after reveal" }
                }
            }),
        },
        ToolDefinition {
            name: "fax_deliver".into(),
            description: "Reveal your hash-lock secret to confirm resource delivery".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["trade_id"],
                "properties": {
                    "trade_id": { "type": "string", "description": "ID of the trade to deliver" }
                }
            }),
        },
        ToolDefinition {
            name: "fax_anchor".into(),
            description: "Anchor the trade's VC chain hash on L2 blockchain".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["trade_id"],
                "properties": {
                    "trade_id": { "type": "string", "description": "ID of the completed trade to anchor" }
                }
            }),
        },
        ToolDefinition {
            name: "fax_check_reputation".into(),
            description: "Check an agent's on-chain reputation score before trading".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["agent_address"],
                "properties": {
                    "agent_address": { "type": "string", "description": "EVM address or DID of the agent" }
                }
            }),
        },
        ToolDefinition {
            name: "fax_list_trades".into(),
            description: "List active and recent FAX trades for this agent".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string", "enum": ["active", "completed", "all"], "description": "Filter by status" }
                }
            }),
        },
        ToolDefinition {
            name: "fax_rates".into(),
            description: "Show current RCU exchange rates for resource types".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "resource_type": { "type": "string", "description": "Specific resource type, or empty for all" }
                }
            }),
        },
    ]
}

/// Simulated tool definition matching OpenFang's format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Result from an FAX tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ToolResult {
    pub fn ok(output: impl Into<String>) -> Self {
        Self { success: true, output: output.into(), data: None }
    }

    pub fn ok_with_data(output: impl Into<String>, data: Value) -> Self {
        Self { success: true, output: output.into(), data: Some(data) }
    }

    pub fn err(output: impl Into<String>) -> Self {
        Self { success: false, output: output.into(), data: None }
    }
}

/// The FAX tool executor — dispatches tool calls and enforces capabilities.
pub struct FaxToolRunner {
    pub agent_did: String,
    pub capabilities: Vec<FaxCapability>,
    pub trade_manager: TradeManager,
    pub discovery: DiscoveryService,
    pub chain_client: ChainClient,
    pub reputation: ReputationService,
}

impl FaxToolRunner {
    pub fn new(
        agent_did: impl Into<String>,
        capabilities: Vec<FaxCapability>,
        chain_config: fax_chain::ChainConfig,
    ) -> Self {
        let did = agent_did.into();
        Self {
            trade_manager: TradeManager::new(&did),
            agent_did: did,
            capabilities,
            discovery: DiscoveryService::new(),
            chain_client: ChainClient::new(chain_config),
            reputation: ReputationService::new(),
        }
    }

    /// Execute an FAX tool by name. This is the entry point wired into
    /// OpenFang's `tool_runner.rs` match block.
    pub async fn execute(&mut self, tool_name: &str, input: Value) -> ToolResult {
        match tool_name {
            "fax_discover" => self.tool_discover(input).await,
            "fax_create_offer" => self.tool_create_offer(input).await,
            "fax_accept_offer" => self.tool_accept_offer(input).await,
            "fax_lock_resource" => self.tool_lock_resource(input).await,
            "fax_deliver" => self.tool_deliver(input).await,
            "fax_anchor" => self.tool_anchor(input).await,
            "fax_check_reputation" => self.tool_check_reputation(input).await,
            "fax_list_trades" => self.tool_list_trades(input).await,
            "fax_rates" => self.tool_rates(input).await,
            _ => ToolResult::err(format!("unknown FAX tool: {tool_name}")),
        }
    }

    async fn tool_discover(&self, input: Value) -> ToolResult {
        if !capability_check(&self.capabilities, &FaxCapability::FaxDiscover) {
            return ToolResult::err("FaxDiscover capability not granted");
        }

        let mut query = DiscoveryQuery::new();
        if let Some(needs) = input.get("needs_resource").and_then(|v| v.as_str()) {
            if let Some(rtype) = parse_resource_type(needs) {
                query = query.needs(rtype);
            }
        }
        if let Some(min_rep) = input.get("min_reputation").and_then(|v| v.as_u64()) {
            query = query.min_reputation(min_rep);
        }

        let results = self.discovery.query(&query);
        let agents: Vec<Value> = results.iter().map(|a| {
            serde_json::json!({
                "did": a.did,
                "name": a.name,
                "resources": a.offered_resource_types,
                "reputation": a.reputation_score,
                "security": format!("{:?}", a.min_security_level),
            })
        }).collect();

        ToolResult::ok_with_data(
            format!("Found {} FAX-capable agents", agents.len()),
            Value::Array(agents),
        )
    }

    async fn tool_create_offer(&mut self, input: Value) -> ToolResult {
        let offer_type = match input.get("offer_type").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::err("missing offer_type"),
        };
        let offer_amount = input.get("offer_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let offer_unit = input.get("offer_unit").and_then(|v| v.as_str()).unwrap_or("unit");
        let request_type = match input.get("request_type").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return ToolResult::err("missing request_type"),
        };
        let request_amount = input.get("request_amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let request_unit = input.get("request_unit").and_then(|v| v.as_str()).unwrap_or("unit");

        if !capability_check(&self.capabilities, &FaxCapability::FaxOffer(offer_type.into())) {
            return ToolResult::err(format!("FaxOffer capability not granted for '{offer_type}'"));
        }

        let offer_rtype = match parse_resource_type(offer_type) {
            Some(r) => r,
            None => return ToolResult::err(format!("unknown resource type: {offer_type}")),
        };
        let request_rtype = match parse_resource_type(request_type) {
            Some(r) => r,
            None => return ToolResult::err(format!("unknown resource type: {request_type}")),
        };

        let trade = self.trade_manager.create_trade();
        let offered = vec![ResourceAmount::new(offer_rtype, offer_amount, offer_unit)];
        let requested = vec![ResourceAmount::new(request_rtype, request_amount, request_unit)];

        match trade.create_offer(offered, requested) {
            Ok(credential) => {
                let trade_id = trade.id.clone();
                ToolResult::ok_with_data(
                    format!("Created trade offer {trade_id}"),
                    serde_json::json!({
                        "trade_id": trade_id,
                        "credential_id": credential.id,
                        "rcu_value": trade.rcu_value,
                    }),
                )
            }
            Err(e) => ToolResult::err(format!("failed to create offer: {e}")),
        }
    }

    async fn tool_accept_offer(&mut self, input: Value) -> ToolResult {
        if !capability_check(&self.capabilities, &FaxCapability::FaxAccept) {
            return ToolResult::err("FaxAccept capability not granted");
        }

        let trade_id = match input.get("trade_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => return ToolResult::err("missing trade_id"),
        };
        let security_level = input.get("security_level").and_then(|v| v.as_u64()).unwrap_or(2);
        let sec = match security_level {
            0 => SecurityLevel::Trust,
            1 => SecurityLevel::Anchor,
            2 => SecurityLevel::Escrow,
            3 => SecurityLevel::FullEscrow,
            _ => SecurityLevel::Escrow,
        };

        ToolResult::ok_with_data(
            format!("Accepted trade {trade_id} at security level {sec}"),
            serde_json::json!({ "trade_id": trade_id, "security_level": security_level }),
        )
    }

    async fn tool_lock_resource(&mut self, input: Value) -> ToolResult {
        let trade_id = match input.get("trade_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => return ToolResult::err("missing trade_id"),
        };
        let endpoint = input.get("resource_endpoint").and_then(|v| v.as_str()).unwrap_or("local");

        let trade = match self.trade_manager.get_trade_mut(&trade_id) {
            Some(t) => t,
            None => return ToolResult::err(format!("trade {trade_id} not found")),
        };

        match trade.begin_locking(endpoint) {
            Ok(credential) => ToolResult::ok_with_data(
                format!("Locked resource for trade {trade_id}"),
                serde_json::json!({
                    "credential_id": credential.id,
                    "endpoint": endpoint,
                }),
            ),
            Err(e) => ToolResult::err(format!("failed to lock: {e}")),
        }
    }

    async fn tool_deliver(&mut self, input: Value) -> ToolResult {
        let trade_id = match input.get("trade_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => return ToolResult::err("missing trade_id"),
        };

        let trade = match self.trade_manager.get_trade_mut(&trade_id) {
            Some(t) => t,
            None => return ToolResult::err(format!("trade {trade_id} not found")),
        };

        match trade.deliver() {
            Ok(credential) => ToolResult::ok_with_data(
                format!("Delivered resource for trade {trade_id} — secret revealed"),
                serde_json::json!({ "credential_id": credential.id }),
            ),
            Err(e) => ToolResult::err(format!("failed to deliver: {e}")),
        }
    }

    async fn tool_anchor(&mut self, input: Value) -> ToolResult {
        if !capability_check(&self.capabilities, &FaxCapability::FaxAnchor) {
            return ToolResult::err("FaxAnchor capability not granted");
        }

        let trade_id = match input.get("trade_id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => return ToolResult::err("missing trade_id"),
        };

        let trade = match self.trade_manager.get_trade_mut(&trade_id) {
            Some(t) => t,
            None => return ToolResult::err(format!("trade {trade_id} not found")),
        };

        match trade.finalize() {
            Ok((_completion, tip_hash)) => {
                let evm_addr = format!("0x{}", &sha2_hex(self.agent_did.as_bytes())[..40]);
                match self.chain_client.anchor_hash(&evm_addr, &tip_hash).await {
                    Ok(receipt) => {
                        trade.set_anchored(receipt.tx_hash.clone());
                        ToolResult::ok_with_data(
                            format!("Anchored trade {trade_id} on L2 block #{}", receipt.block_number),
                            serde_json::json!({
                                "tx_hash": receipt.tx_hash,
                                "block_number": receipt.block_number,
                                "chain_tip_hash": tip_hash,
                            }),
                        )
                    }
                    Err(e) => ToolResult::err(format!("anchor failed: {e}")),
                }
            }
            Err(e) => ToolResult::err(format!("finalize failed: {e}")),
        }
    }

    async fn tool_check_reputation(&self, input: Value) -> ToolResult {
        let agent_addr = match input.get("agent_address").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return ToolResult::err("missing agent_address"),
        };

        let score = self.reputation.get_score(agent_addr);
        let registered = self.reputation.is_registered(agent_addr);

        ToolResult::ok_with_data(
            format!("Agent {agent_addr}: score={score}/1000, registered={registered}"),
            serde_json::json!({
                "address": agent_addr,
                "score": score,
                "registered": registered,
                "reputation": self.reputation.get_reputation(agent_addr),
            }),
        )
    }

    async fn tool_list_trades(&self, input: Value) -> ToolResult {
        let filter = input.get("status").and_then(|v| v.as_str()).unwrap_or("all");

        let trades: Vec<Value> = self.trade_manager.active_trades.iter()
            .filter(|(_, t)| match filter {
                "active" => t.phase != TradePhase::Complete,
                "completed" => t.phase == TradePhase::Complete,
                _ => true,
            })
            .map(|(id, t)| serde_json::json!({
                "id": id,
                "phase": format!("{:?}", t.phase),
                "counterparty": t.counterparty_did,
                "rcu_value": t.rcu_value,
                "security": format!("{:?}", t.security_level),
            }))
            .collect();

        ToolResult::ok_with_data(
            format!("{} trades found", trades.len()),
            Value::Array(trades),
        )
    }

    async fn tool_rates(&self, input: Value) -> ToolResult {
        let specific = input.get("resource_type").and_then(|v| v.as_str());

        let all_types = vec![
            ("compute", ResourceType::Compute, "gpu-hour", Some("gpu-hour")),
            ("llm_tokens", ResourceType::LlmTokens, "tokens", None),
            ("knowledge_access", ResourceType::KnowledgeAccess, "query", None),
            ("tool_access", ResourceType::ToolAccess, "invocation", None),
            ("research_report", ResourceType::ResearchReport, "report", None),
            ("storage_quota", ResourceType::StorageQuota, "MB-month", None),
            ("bandwidth", ResourceType::Bandwidth, "GB", None),
        ];

        let rates: Vec<Value> = all_types.iter()
            .filter(|(name, ..)| specific.is_none() || specific == Some(*name))
            .map(|(name, rtype, unit, subtype)| {
                let r = ResourceAmount {
                    resource_type: rtype.clone(),
                    amount: 1.0,
                    unit: unit.to_string(),
                    subtype: subtype.map(|s| s.to_string()),
                };
                let rcu = RcuOracle::to_rcu(&r).unwrap_or(0.0);
                serde_json::json!({
                    "resource": name,
                    "unit": unit,
                    "rcu_per_unit": rcu,
                })
            })
            .collect();

        ToolResult::ok_with_data("RCU exchange rates", Value::Array(rates))
    }
}

fn parse_resource_type(s: &str) -> Option<ResourceType> {
    match s.to_lowercase().as_str() {
        "compute" => Some(ResourceType::Compute),
        "llm_tokens" => Some(ResourceType::LlmTokens),
        "knowledge_access" => Some(ResourceType::KnowledgeAccess),
        "tool_access" => Some(ResourceType::ToolAccess),
        "research_report" => Some(ResourceType::ResearchReport),
        "data_feed" => Some(ResourceType::DataFeed),
        "schedule_slot" => Some(ResourceType::ScheduleSlot),
        "storage_quota" => Some(ResourceType::StorageQuota),
        "bandwidth" => Some(ResourceType::Bandwidth),
        "attestation" => Some(ResourceType::Attestation),
        other => Some(ResourceType::Custom(other.to_string())),
    }
}

fn sha2_hex(data: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    hex::encode(Sha256::digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_count() {
        let defs = fax_tool_definitions();
        assert_eq!(defs.len(), 9);
        assert!(defs.iter().any(|d| d.name == "fax_discover"));
        assert!(defs.iter().any(|d| d.name == "fax_create_offer"));
        assert!(defs.iter().any(|d| d.name == "fax_anchor"));
    }

    #[tokio::test]
    async fn test_tool_runner_create_offer() {
        let mut runner = FaxToolRunner::new(
            "did:wba:test.com:user:alpha",
            vec![
                FaxCapability::FaxOffer("*".into()),
                FaxCapability::FaxAccept,
                FaxCapability::FaxDiscover,
            ],
            fax_chain::ChainConfig::local(),
        );

        let result = runner.execute("fax_create_offer", serde_json::json!({
            "counterparty_did": "did:wba:other.com:user:beta",
            "offer_type": "compute",
            "offer_amount": 2.0,
            "offer_unit": "gpu-hour",
            "request_type": "llm_tokens",
            "request_amount": 100000.0,
            "request_unit": "tokens"
        })).await;

        assert!(result.success, "tool failed: {}", result.output);
        assert!(result.data.is_some());
    }

    #[tokio::test]
    async fn test_tool_runner_capability_denied() {
        let mut runner = FaxToolRunner::new(
            "did:wba:test.com:user:alpha",
            vec![], // no capabilities
            fax_chain::ChainConfig::local(),
        );

        let result = runner.execute("fax_discover", serde_json::json!({})).await;
        assert!(!result.success);
        assert!(result.output.contains("capability not granted"));
    }

    #[tokio::test]
    async fn test_tool_runner_rates() {
        let mut runner = FaxToolRunner::new(
            "did:wba:test.com:user:alpha",
            vec![],
            fax_chain::ChainConfig::local(),
        );

        let result = runner.execute("fax_rates", serde_json::json!({})).await;
        assert!(result.success);
        assert!(result.data.is_some());
    }
}
