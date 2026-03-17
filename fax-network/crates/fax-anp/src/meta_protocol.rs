use serde::{Deserialize, Serialize};
use fax_types::SecurityLevel;

/// FAX protocol URI for ANP meta-protocol negotiation.
pub const FAX_PROTOCOL_URI: &str = "https://fax-network.org/protocol/1.0";

/// ANP meta-protocol message types.
/// These map to §6 of the ANP spec — the binary header PT=00 negotiation messages.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum ProtocolType {
    MetaProtocol = 0,
    ApplicationProtocol = 1,
    NaturalLanguage = 2,
    Verification = 3,
}

/// A meta-protocol negotiation message for selecting FAX.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaProtocolNegotiation {
    pub action: String,
    pub sequence_id: u32,
    pub candidate_protocols: Vec<String>,
    pub status: NegotiationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modification_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fax_params: Option<FaxNegotiationParams>,
}

/// FAX-specific parameters exchanged during meta-protocol negotiation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaxNegotiationParams {
    pub supported_resource_types: Vec<String>,
    pub min_security_level: u8,
    pub max_security_level: u8,
    pub supports_blockchain_anchor: bool,
    pub supports_escrow: bool,
    pub reputation_score: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NegotiationStatus {
    Negotiating,
    Accepted,
    Rejected,
    Timeout,
}

impl MetaProtocolNegotiation {
    /// Create an initial proposal that includes FAX in the candidate list.
    pub fn propose_fax(
        sequence_id: u32,
        additional_protocols: Vec<String>,
        fax_params: FaxNegotiationParams,
    ) -> Self {
        let mut candidates = vec![FAX_PROTOCOL_URI.to_string()];
        candidates.extend(additional_protocols);

        Self {
            action: "protocolNegotiation".into(),
            sequence_id,
            candidate_protocols: candidates,
            status: NegotiationStatus::Negotiating,
            modification_summary: None,
            fax_params: Some(fax_params),
        }
    }

    /// Accept FAX as the selected protocol.
    pub fn accept_fax(sequence_id: u32) -> Self {
        Self {
            action: "protocolNegotiation".into(),
            sequence_id,
            candidate_protocols: vec![FAX_PROTOCOL_URI.to_string()],
            status: NegotiationStatus::Accepted,
            modification_summary: Some("FAX resource trading protocol selected".into()),
            fax_params: None,
        }
    }

    /// Reject the protocol negotiation.
    pub fn reject(sequence_id: u32, reason: impl Into<String>) -> Self {
        Self {
            action: "protocolNegotiation".into(),
            sequence_id,
            candidate_protocols: Vec::new(),
            status: NegotiationStatus::Rejected,
            modification_summary: Some(reason.into()),
            fax_params: None,
        }
    }

    /// Check if the counterparty's proposal includes FAX.
    pub fn includes_fax(&self) -> bool {
        self.candidate_protocols.iter().any(|p| p == FAX_PROTOCOL_URI)
    }

    /// Check if FAX parameters are compatible with our requirements.
    pub fn fax_params_compatible(
        ours: &FaxNegotiationParams,
        theirs: &FaxNegotiationParams,
    ) -> bool {
        let security_overlap = ours.min_security_level <= theirs.max_security_level
            && theirs.min_security_level <= ours.max_security_level;

        if !security_overlap {
            return false;
        }

        let resource_overlap = ours.supported_resource_types.iter()
            .any(|r| theirs.supported_resource_types.contains(r));

        resource_overlap
    }
}

impl FaxNegotiationParams {
    pub fn new(
        resource_types: Vec<String>,
        min_sec: SecurityLevel,
        max_sec: SecurityLevel,
    ) -> Self {
        Self {
            supported_resource_types: resource_types,
            min_security_level: min_sec as u8,
            max_security_level: max_sec as u8,
            supports_blockchain_anchor: min_sec >= SecurityLevel::Anchor,
            supports_escrow: min_sec >= SecurityLevel::Escrow,
            reputation_score: None,
        }
    }

    pub fn with_reputation(mut self, score: u64) -> Self {
        self.reputation_score = Some(score);
        self
    }
}

/// Encode an ANP binary header for a message.
/// Format: [PT(2 bits) | reserved(6 bits)] [payload...]
pub fn encode_anp_header(protocol_type: ProtocolType) -> u8 {
    (protocol_type as u8) << 6
}

/// Decode the protocol type from an ANP binary header byte.
pub fn decode_anp_header(byte: u8) -> ProtocolType {
    match byte >> 6 {
        0 => ProtocolType::MetaProtocol,
        1 => ProtocolType::ApplicationProtocol,
        2 => ProtocolType::NaturalLanguage,
        3 => ProtocolType::Verification,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_propose_and_accept() {
        let params = FaxNegotiationParams::new(
            vec!["compute".into(), "llm_tokens".into()],
            SecurityLevel::Anchor,
            SecurityLevel::FullEscrow,
        );
        let proposal = MetaProtocolNegotiation::propose_fax(0, vec![], params);
        assert!(proposal.includes_fax());
        assert_eq!(proposal.status, NegotiationStatus::Negotiating);

        let accept = MetaProtocolNegotiation::accept_fax(1);
        assert!(accept.includes_fax());
        assert_eq!(accept.status, NegotiationStatus::Accepted);
    }

    #[test]
    fn test_params_compatible() {
        let ours = FaxNegotiationParams::new(
            vec!["compute".into(), "llm_tokens".into()],
            SecurityLevel::Anchor,
            SecurityLevel::FullEscrow,
        );
        let theirs = FaxNegotiationParams::new(
            vec!["llm_tokens".into(), "knowledge_access".into()],
            SecurityLevel::Escrow,
            SecurityLevel::Escrow,
        );
        assert!(MetaProtocolNegotiation::fax_params_compatible(&ours, &theirs));

        let incompatible = FaxNegotiationParams::new(
            vec!["bandwidth".into()],
            SecurityLevel::Escrow,
            SecurityLevel::Escrow,
        );
        assert!(!MetaProtocolNegotiation::fax_params_compatible(&ours, &incompatible));
    }

    #[test]
    fn test_anp_header_roundtrip() {
        for pt in [
            ProtocolType::MetaProtocol,
            ProtocolType::ApplicationProtocol,
            ProtocolType::NaturalLanguage,
            ProtocolType::Verification,
        ] {
            let encoded = encode_anp_header(pt.clone());
            let decoded = decode_anp_header(encoded);
            assert_eq!(decoded, pt);
        }
    }
}
