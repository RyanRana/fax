use fax_types::FaxResult;
use serde::{Deserialize, Serialize};

/// Configuration for connecting to the FAX smart contracts on L2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub anchor_contract: String,
    pub escrow_contract: String,
    pub reputation_contract: String,
    pub private_key: Option<String>,
}

impl ChainConfig {
    /// Arbitrum Sepolia testnet configuration (default for development).
    pub fn arbitrum_sepolia() -> Self {
        Self {
            rpc_url: "https://sepolia-rollup.arbitrum.io/rpc".into(),
            chain_id: 421614,
            anchor_contract: String::new(),
            escrow_contract: String::new(),
            reputation_contract: String::new(),
            private_key: None,
        }
    }

    /// Local development configuration (Anvil/Hardhat).
    pub fn local() -> Self {
        Self {
            rpc_url: "http://127.0.0.1:8545".into(),
            chain_id: 31337,
            anchor_contract: String::new(),
            escrow_contract: String::new(),
            reputation_contract: String::new(),
            private_key: None,
        }
    }

    pub fn with_contracts(
        mut self,
        anchor: impl Into<String>,
        escrow: impl Into<String>,
        reputation: impl Into<String>,
    ) -> Self {
        self.anchor_contract = anchor.into();
        self.escrow_contract = escrow.into();
        self.reputation_contract = reputation.into();
        self
    }

    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.private_key = Some(key.into());
        self
    }
}

/// Simulated chain client for development and testing.
/// In production, this would use ethers-rs or alloy to interact with the L2.
pub struct ChainClient {
    pub config: ChainConfig,
    anchors: std::collections::HashMap<String, Vec<AnchorEntry>>,
    next_block: u64,
}

#[derive(Debug, Clone)]
pub struct AnchorEntry {
    pub chain_hash: String,
    pub block_number: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct TxReceipt {
    pub tx_hash: String,
    pub block_number: u64,
    pub status: bool,
}

impl ChainClient {
    pub fn new(config: ChainConfig) -> Self {
        Self {
            config,
            anchors: std::collections::HashMap::new(),
            next_block: 1,
        }
    }

    /// Simulate anchoring a VC chain hash on L2.
    pub async fn anchor_hash(&mut self, agent_address: &str, chain_hash: &str) -> FaxResult<TxReceipt> {
        let block = self.next_block;
        self.next_block += 1;

        let entry = AnchorEntry {
            chain_hash: chain_hash.to_string(),
            block_number: block,
            timestamp: chrono::Utc::now().timestamp() as u64,
        };

        self.anchors
            .entry(agent_address.to_string())
            .or_default()
            .push(entry);

        let tx_hash = format!("0x{}", fax_types::sha256_hex(
            format!("anchor:{agent_address}:{chain_hash}:{block}").as_bytes()
        ));

        tracing::info!(
            agent = agent_address,
            hash = chain_hash,
            block = block,
            tx = %tx_hash,
            "anchored VC chain hash on L2"
        );

        Ok(TxReceipt {
            tx_hash,
            block_number: block,
            status: true,
        })
    }

    /// Verify an anchor exists on-chain.
    pub async fn verify_anchor(&self, agent_address: &str, chain_hash: &str) -> FaxResult<Option<AnchorEntry>> {
        let entries = self.anchors.get(agent_address);
        Ok(entries.and_then(|v| v.iter().find(|e| e.chain_hash == chain_hash).cloned()))
    }

    /// Get the latest anchor for an agent.
    pub async fn get_latest_anchor(&self, agent_address: &str) -> FaxResult<Option<AnchorEntry>> {
        Ok(self.anchors.get(agent_address).and_then(|v| v.last().cloned()))
    }

    /// Get anchor count for an agent.
    pub async fn get_anchor_count(&self, agent_address: &str) -> u64 {
        self.anchors.get(agent_address).map(|v| v.len() as u64).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_anchor_and_verify() {
        let mut client = ChainClient::new(ChainConfig::local());
        let receipt = client.anchor_hash("0xAlice", "abc123hash").await.unwrap();
        assert!(receipt.status);
        assert!(!receipt.tx_hash.is_empty());

        let found = client.verify_anchor("0xAlice", "abc123hash").await.unwrap();
        assert!(found.is_some());

        let missing = client.verify_anchor("0xAlice", "wronghash").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_anchor_sequence() {
        let mut client = ChainClient::new(ChainConfig::local());
        client.anchor_hash("0xAlice", "hash1").await.unwrap();
        client.anchor_hash("0xAlice", "hash2").await.unwrap();
        client.anchor_hash("0xAlice", "hash3").await.unwrap();

        assert_eq!(client.get_anchor_count("0xAlice").await, 3);
        let latest = client.get_latest_anchor("0xAlice").await.unwrap().unwrap();
        assert_eq!(latest.chain_hash, "hash3");
    }
}
