use fax_types::*;
use fax_protocol::*;
use serde::{Deserialize, Serialize};

use crate::meta_protocol::{encode_anp_header, ProtocolType};

/// FAX application-layer messages sent over ANP's WebSocket channel (PT=01).
/// Once the meta-protocol selects FAX, all trade messages use this envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FaxMessage {
    /// Agent advertises available resources and rates.
    ResourceAdvertisement {
        from_did: String,
        resources: Vec<TradableResource>,
        exchange_rates: Vec<ExchangeRate>,
    },

    /// Propose a trade (CredentialSubject::ResourceOffer wrapped in transport).
    TradeProposal {
        credential: FaxCredential,
        security_proposal: NegotiationMessage,
    },

    /// Counter-offer during negotiation.
    TradeCounter {
        credential: FaxCredential,
        security_proposal: NegotiationMessage,
    },

    /// Accept the trade terms — carries the SwapAgreement credential.
    TradeAccept {
        credential: FaxCredential,
    },

    /// Reject the trade.
    TradeReject {
        trade_id: String,
        reason: String,
    },

    /// Resource lock — carries the ResourceLock credential + hash-lock.
    ResourceLock {
        credential: FaxCredential,
    },

    /// Resource delivery — reveals the hash-lock secret.
    ResourceDelivery {
        credential: FaxCredential,
    },

    /// Trade completion — both parties confirmed.
    TradeComplete {
        credential: FaxCredential,
        chain_tip_hash: String,
    },

    /// Anchor receipt — confirms the chain hash was posted to L2.
    AnchorReceipt {
        credential: FaxCredential,
        tx_hash: String,
        block_number: u64,
    },

    /// Dispute initiation.
    Dispute {
        credential: FaxCredential,
    },

    /// Heartbeat / keepalive.
    Ping {
        timestamp: u64,
    },

    /// Acknowledgment.
    Pong {
        timestamp: u64,
    },
}

/// Framed message ready for ANP WebSocket transport.
/// The first byte is the ANP binary header (PT=01 for application protocol).
#[derive(Debug)]
pub struct AnpFrame {
    pub header: u8,
    pub payload: Vec<u8>,
}

impl AnpFrame {
    /// Frame an FAX message for ANP transport.
    pub fn from_fax_message(msg: &FaxMessage) -> FaxResult<Self> {
        let json = serde_json::to_vec(msg)
            .map_err(|e| FaxError::SerializationError(e.to_string()))?;

        Ok(Self {
            header: encode_anp_header(ProtocolType::ApplicationProtocol),
            payload: json,
        })
    }

    /// Parse an FAX message from a raw ANP frame.
    pub fn to_fax_message(data: &[u8]) -> FaxResult<FaxMessage> {
        if data.is_empty() {
            return Err(FaxError::Other("empty frame".into()));
        }

        // Skip the header byte
        let payload = if data.len() > 1 { &data[1..] } else { data };

        serde_json::from_slice(payload)
            .map_err(|e| FaxError::SerializationError(e.to_string()))
    }

    /// Serialize the frame to bytes (header + payload).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + self.payload.len());
        buf.push(self.header);
        buf.extend_from_slice(&self.payload);
        buf
    }
}

/// Session managing the FAX message flow over an ANP connection.
pub struct FaxSession {
    pub my_did: String,
    pub peer_did: String,
    pub trade_id: Option<String>,
    pub state: SessionState,
    outbound_queue: Vec<FaxMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Connected,
    Negotiating,
    Trading,
    Locking,
    Exchanging,
    Anchoring,
    Complete,
    Failed,
}

impl FaxSession {
    pub fn new(my_did: impl Into<String>, peer_did: impl Into<String>) -> Self {
        Self {
            my_did: my_did.into(),
            peer_did: peer_did.into(),
            trade_id: None,
            state: SessionState::Connected,
            outbound_queue: Vec::new(),
        }
    }

    /// Queue a message for sending.
    pub fn send(&mut self, msg: FaxMessage) {
        match &msg {
            FaxMessage::TradeProposal { credential, .. } => {
                self.trade_id = Some(credential.trade_id().to_string());
                self.state = SessionState::Negotiating;
            }
            FaxMessage::TradeAccept { .. } => {
                self.state = SessionState::Trading;
            }
            FaxMessage::ResourceLock { .. } => {
                self.state = SessionState::Locking;
            }
            FaxMessage::ResourceDelivery { .. } => {
                self.state = SessionState::Exchanging;
            }
            FaxMessage::TradeComplete { .. } => {
                self.state = SessionState::Anchoring;
            }
            FaxMessage::AnchorReceipt { .. } => {
                self.state = SessionState::Complete;
            }
            FaxMessage::TradeReject { .. } => {
                self.state = SessionState::Failed;
            }
            _ => {}
        }
        self.outbound_queue.push(msg);
    }

    /// Drain pending outbound messages.
    pub fn drain_outbound(&mut self) -> Vec<FaxMessage> {
        std::mem::take(&mut self.outbound_queue)
    }

    /// Process an inbound message and update session state.
    pub fn receive(&mut self, msg: &FaxMessage) {
        match msg {
            FaxMessage::TradeProposal { credential, .. } => {
                self.trade_id = Some(credential.trade_id().to_string());
                self.state = SessionState::Negotiating;
            }
            FaxMessage::TradeAccept { .. } => {
                self.state = SessionState::Trading;
            }
            FaxMessage::ResourceLock { .. } => {
                self.state = SessionState::Locking;
            }
            FaxMessage::ResourceDelivery { .. } => {
                self.state = SessionState::Exchanging;
            }
            FaxMessage::TradeComplete { .. } => {
                self.state = SessionState::Anchoring;
            }
            FaxMessage::AnchorReceipt { .. } => {
                self.state = SessionState::Complete;
            }
            FaxMessage::TradeReject { .. } => {
                self.state = SessionState::Failed;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_frame_roundtrip() {
        let msg = FaxMessage::Ping { timestamp: 12345 };
        let frame = AnpFrame::from_fax_message(&msg).unwrap();
        assert_eq!(frame.header >> 6, 1); // PT=01 (application protocol)

        let bytes = frame.to_bytes();
        let decoded = AnpFrame::to_fax_message(&bytes).unwrap();
        if let FaxMessage::Ping { timestamp } = decoded {
            assert_eq!(timestamp, 12345);
        } else {
            panic!("wrong message type");
        }
    }

    #[test]
    fn test_session_state_transitions() {
        let mut session = FaxSession::new("did:wba:a.com:user:alice", "did:wba:b.com:user:bob");
        assert_eq!(session.state, SessionState::Connected);

        let offer = FaxCredential::new(
            CredentialType::ResourceOffer,
            "did:wba:a.com:user:alice".into(),
            CredentialSubject::ResourceOffer {
                trade_id: "trade-001".into(),
                offered: vec![],
                requested: vec![],
                rcu_value: 100.0,
                expiry: Utc::now() + chrono::Duration::hours(1),
            },
        );

        let proposal = FaxMessage::TradeProposal {
            credential: offer,
            security_proposal: NegotiationMessage {
                trade_id: "trade-001".into(),
                from_did: "did:wba:a.com:user:alice".into(),
                to_did: "did:wba:b.com:user:bob".into(),
                action: NegotiationAction::Propose {
                    offered: vec![],
                    requested: vec![],
                    rcu_value: 100.0,
                    message: None,
                },
                security_proposal: fax_protocol::SecurityProposal::new(SecurityLevel::Escrow),
            },
        };

        session.send(proposal);
        assert_eq!(session.state, SessionState::Negotiating);
        assert_eq!(session.trade_id, Some("trade-001".into()));
        assert_eq!(session.drain_outbound().len(), 1);
    }
}
