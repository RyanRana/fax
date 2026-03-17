use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    Compute,
    LlmTokens,
    KnowledgeAccess,
    ToolAccess,
    ResearchReport,
    DataFeed,
    ScheduleSlot,
    StorageQuota,
    Bandwidth,
    Attestation,
    Custom(String),
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compute => write!(f, "compute"),
            Self::LlmTokens => write!(f, "llm_tokens"),
            Self::KnowledgeAccess => write!(f, "knowledge_access"),
            Self::ToolAccess => write!(f, "tool_access"),
            Self::ResearchReport => write!(f, "research_report"),
            Self::DataFeed => write!(f, "data_feed"),
            Self::ScheduleSlot => write!(f, "schedule_slot"),
            Self::StorageQuota => write!(f, "storage_quota"),
            Self::Bandwidth => write!(f, "bandwidth"),
            Self::Attestation => write!(f, "attestation"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAmount {
    pub resource_type: ResourceType,
    pub amount: f64,
    pub unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
}

impl ResourceAmount {
    pub fn new(resource_type: ResourceType, amount: f64, unit: impl Into<String>) -> Self {
        Self {
            resource_type,
            amount,
            unit: unit.into(),
            subtype: None,
        }
    }

    pub fn with_subtype(mut self, subtype: impl Into<String>) -> Self {
        self.subtype = Some(subtype.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityDescriptor {
    pub attributes: serde_json::Map<String, serde_json::Value>,
}

impl QualityDescriptor {
    pub fn new() -> Self {
        Self {
            attributes: serde_json::Map::new(),
        }
    }

    pub fn set(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

impl Default for QualityDescriptor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradableResource {
    pub resource: ResourceAmount,
    pub quality: Option<QualityDescriptor>,
    pub min_trade: Option<f64>,
    pub max_trade: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability_windows: Option<Vec<AvailabilityWindow>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailabilityWindow {
    pub days: Vec<String>,
    pub start_utc: String,
    pub end_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRate {
    pub give: ResourceAmount,
    pub receive: ResourceAmount,
    pub rate_type: RateType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateType {
    Fixed,
    Indicative,
    Negotiable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResourceProfile {
    pub agent_did: String,
    pub offered_resources: Vec<TradableResource>,
    pub accepted_resource_types: Vec<ResourceType>,
    pub exchange_rates: Vec<ExchangeRate>,
    pub trading_policy: TradingPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingPolicy {
    pub min_security_level: SecurityLevel,
    pub requires_blockchain_anchor: bool,
    pub accepts_credit: bool,
    pub max_concurrent_trades: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispute_resolution_did: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum SecurityLevel {
    Trust = 0,
    Anchor = 1,
    Escrow = 2,
    FullEscrow = 3,
    ZkPrivate = 4,
}

impl fmt::Display for SecurityLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trust => write!(f, "Level 0: Trust (VC chain only)"),
            Self::Anchor => write!(f, "Level 1: Anchor (VC chain + L2 hash)"),
            Self::Escrow => write!(f, "Level 2: Escrow (on-chain hash-lock)"),
            Self::FullEscrow => write!(f, "Level 3: Full Escrow (+ arbitration)"),
            Self::ZkPrivate => write!(f, "Level 4: ZK Private (+ selective disclosure)"),
        }
    }
}
