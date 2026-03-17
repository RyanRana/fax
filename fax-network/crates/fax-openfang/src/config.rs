use serde::{Deserialize, Serialize};

use crate::capabilities::FaxManifestCapabilities;

/// FAX configuration section for OpenFang's KernelConfig.
/// Add this as `pub fax: Option<FaxConfig>` in KernelConfig.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaxConfig {
    pub enabled: bool,
    pub chain: FaxChainConfig,
    pub trading: FaxTradingConfig,
    pub discovery: FaxDiscoveryConfig,
    pub capabilities: FaxManifestCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaxChainConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub anchor_contract: String,
    pub escrow_contract: String,
    pub reputation_contract: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaxTradingConfig {
    pub max_concurrent_trades: u32,
    pub default_lock_duration_secs: u64,
    pub default_security_level: u8,
    pub max_rcu_per_trade: f64,
    pub auto_anchor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaxDiscoveryConfig {
    pub known_domains: Vec<String>,
    pub crawl_interval_secs: u64,
    pub cache_ttl_secs: u64,
}

impl Default for FaxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            chain: FaxChainConfig {
                rpc_url: "https://sepolia-rollup.arbitrum.io/rpc".into(),
                chain_id: 421614,
                anchor_contract: String::new(),
                escrow_contract: String::new(),
                reputation_contract: String::new(),
                private_key_env: Some("FAX_PRIVATE_KEY".into()),
            },
            trading: FaxTradingConfig {
                max_concurrent_trades: 5,
                default_lock_duration_secs: 3600,
                default_security_level: 2,
                max_rcu_per_trade: 1000.0,
                auto_anchor: true,
            },
            discovery: FaxDiscoveryConfig {
                known_domains: Vec::new(),
                crawl_interval_secs: 300,
                cache_ttl_secs: 600,
            },
            capabilities: FaxManifestCapabilities::default(),
        }
    }
}

impl FaxConfig {
    /// Generate a TOML config snippet that can be added to OpenFang's config.toml.
    pub fn to_toml_snippet(&self) -> String {
        format!(
r#"[fax]
enabled = {enabled}

[fax.chain]
rpc_url = "{rpc_url}"
chain_id = {chain_id}
anchor_contract = "{anchor}"
escrow_contract = "{escrow}"
reputation_contract = "{reputation}"

[fax.trading]
max_concurrent_trades = {max_trades}
default_lock_duration_secs = {lock_dur}
default_security_level = {sec_level}
max_rcu_per_trade = {max_rcu}
auto_anchor = {auto_anchor}

[fax.discovery]
known_domains = {domains:?}
crawl_interval_secs = {crawl}
cache_ttl_secs = {cache}
"#,
            enabled = self.enabled,
            rpc_url = self.chain.rpc_url,
            chain_id = self.chain.chain_id,
            anchor = self.chain.anchor_contract,
            escrow = self.chain.escrow_contract,
            reputation = self.chain.reputation_contract,
            max_trades = self.trading.max_concurrent_trades,
            lock_dur = self.trading.default_lock_duration_secs,
            sec_level = self.trading.default_security_level,
            max_rcu = self.trading.max_rcu_per_trade,
            auto_anchor = self.trading.auto_anchor,
            domains = self.discovery.known_domains,
            crawl = self.discovery.crawl_interval_secs,
            cache = self.discovery.cache_ttl_secs,
        )
    }

    /// Convert to an fax-chain ChainConfig for the chain client.
    pub fn to_chain_config(&self) -> fax_chain::ChainConfig {
        fax_chain::ChainConfig {
            rpc_url: self.chain.rpc_url.clone(),
            chain_id: self.chain.chain_id,
            anchor_contract: self.chain.anchor_contract.clone(),
            escrow_contract: self.chain.escrow_contract.clone(),
            reputation_contract: self.chain.reputation_contract.clone(),
            private_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FaxConfig::default();
        assert!(config.enabled);
        assert_eq!(config.chain.chain_id, 421614);
        assert_eq!(config.trading.default_security_level, 2);
    }

    #[test]
    fn test_toml_snippet() {
        let config = FaxConfig::default();
        let toml = config.to_toml_snippet();
        assert!(toml.contains("[fax]"));
        assert!(toml.contains("[fax.chain]"));
        assert!(toml.contains("[fax.trading]"));
        assert!(toml.contains("enabled = true"));
    }

    #[test]
    fn test_to_chain_config() {
        let config = FaxConfig::default();
        let chain_config = config.to_chain_config();
        assert_eq!(chain_config.chain_id, 421614);
    }
}
