use fax_types::*;
use fax_protocol::SwapEngine;

use crate::client::{ChainClient, TxReceipt};

/// High-level anchor operations that combine the swap engine with on-chain anchoring.
pub struct AnchorService<'a> {
    client: &'a mut ChainClient,
}

impl<'a> AnchorService<'a> {
    pub fn new(client: &'a mut ChainClient) -> Self {
        Self { client }
    }

    /// Anchor a completed trade's credential chain on L2.
    /// Returns the transaction receipt and creates an AnchorReceipt credential.
    pub async fn anchor_trade(
        &mut self,
        swap: &mut SwapEngine,
        agent_address: &str,
        agent_did: &str,
    ) -> FaxResult<(TxReceipt, FaxCredential)> {
        let chain_hash = swap.chain_tip_hash()
            .ok_or_else(|| FaxError::Other("empty credential chain".into()))?;

        let receipt = self.client.anchor_hash(agent_address, &chain_hash).await?;

        let anchor_credential = FaxCredential::new(
            CredentialType::AnchorReceipt,
            agent_did.to_string(),
            CredentialSubject::AnchorReceipt {
                trade_id: swap.trade_id.clone(),
                chain_hash,
                tx_hash: receipt.tx_hash.clone(),
                block_number: receipt.block_number,
                anchored_at: chrono::Utc::now(),
            },
        );

        swap.chain.append(anchor_credential.clone());

        Ok((receipt, anchor_credential))
    }

    /// Verify that a trade's credential chain matches its on-chain anchor.
    pub async fn verify_trade_anchor(
        &self,
        swap: &SwapEngine,
        agent_address: &str,
    ) -> FaxResult<bool> {
        let chain_hash = swap.chain_tip_hash()
            .ok_or_else(|| FaxError::Other("empty chain".into()))?;

        let anchor = self.client.verify_anchor(agent_address, &chain_hash).await?;
        Ok(anchor.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::ChainConfig;

    #[tokio::test]
    async fn test_anchor_trade() {
        let mut client = ChainClient::new(ChainConfig::local());
        let mut swap = SwapEngine::new("trade-anchor-test", 3600);

        let lock_cred = swap.create_lock_credential(
            "did:wba:test.com:user:alice",
            "wss://alice/compute",
        ).unwrap();

        let mut service = AnchorService::new(&mut client);
        let (receipt, anchor_cred) = service.anchor_trade(
            &mut swap,
            "0xAlice",
            "did:wba:test.com:user:alice",
        ).await.unwrap();

        assert!(receipt.status);
        assert_eq!(anchor_cred.credential_type, CredentialType::AnchorReceipt);
    }
}
