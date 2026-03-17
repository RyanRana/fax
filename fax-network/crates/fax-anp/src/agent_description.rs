use fax_types::*;
use serde::{Deserialize, Serialize};

/// ANP Agent Description with FAX resource trading interface.
/// This extends ANP's standard AD format (§7) to include tradable resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaxAgentDescription {
    pub protocol_type: String,
    pub protocol_version: String,
    #[serde(rename = "type")]
    pub ad_type: String,
    pub url: String,
    pub name: String,
    pub did: String,
    pub owner: Option<AdOwner>,
    pub description: String,
    pub security_definitions: serde_json::Value,
    pub security: String,
    #[serde(rename = "Infomations")]
    pub informations: Vec<AdInformation>,
    pub interfaces: Vec<AdInterface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdOwner {
    #[serde(rename = "type")]
    pub owner_type: String,
    pub name: String,
    pub url: Option<String>,
}

/// An information entry in the AD — used to advertise tradable resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdInformation {
    #[serde(rename = "type")]
    pub info_type: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "fax:resourceType")]
    pub fax_resource_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "fax:rcuRate")]
    pub fax_rcu_rate: Option<f64>,
}

/// An interface entry in the AD.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdInterface {
    #[serde(rename = "type")]
    pub interface_type: String,
    pub protocol: String,
    pub version: String,
    pub url: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub human_authorization: Option<bool>,
}

/// Build an ANP Agent Description that includes FAX trading capabilities.
pub struct AgentDescriptionBuilder {
    did: String,
    name: String,
    description: String,
    domain: String,
    resources: Vec<TradableResource>,
    accepted_types: Vec<ResourceType>,
    security_level: SecurityLevel,
}

impl AgentDescriptionBuilder {
    pub fn new(did: impl Into<String>, name: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            did: did.into(),
            name: name.into(),
            description: String::new(),
            domain: domain.into(),
            resources: Vec::new(),
            accepted_types: Vec::new(),
            security_level: SecurityLevel::Anchor,
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn offer_resource(mut self, resource: TradableResource) -> Self {
        self.resources.push(resource);
        self
    }

    pub fn accept_type(mut self, rtype: ResourceType) -> Self {
        self.accepted_types.push(rtype);
        self
    }

    pub fn security_level(mut self, level: SecurityLevel) -> Self {
        self.security_level = level;
        self
    }

    /// Build the ANP-compatible Agent Description JSON.
    pub fn build(&self) -> FaxAgentDescription {
        let ad_url = format!("https://{}/agents/{}/ad.json",
            self.domain,
            self.name.to_lowercase().replace(' ', "-")
        );
        let fax_url = format!("https://{}/agents/{}/fax-interface.json",
            self.domain,
            self.name.to_lowercase().replace(' ', "-")
        );

        let mut informations: Vec<AdInformation> = Vec::new();

        for resource in &self.resources {
            let rcu_rate = RcuOracle::to_rcu(&resource.resource).unwrap_or(0.0)
                / resource.resource.amount.max(1.0);
            informations.push(AdInformation {
                info_type: "Product".into(),
                name: format!("{} ({})", resource.resource.resource_type, resource.resource.unit),
                description: format!(
                    "Tradable resource: up to {:.1} {} available",
                    resource.resource.amount, resource.resource.unit
                ),
                url: None,
                fax_resource_type: Some(resource.resource.resource_type.to_string()),
                fax_rcu_rate: Some(rcu_rate),
            });
        }

        let mut interfaces = vec![
            AdInterface {
                interface_type: "StructuredInterface".into(),
                protocol: "FAX".into(),
                version: "1.0".into(),
                url: fax_url,
                description: "FAX resource trading protocol — hash-lock atomic swaps with L2 anchoring".into(),
                human_authorization: None,
            },
        ];

        if self.security_level >= SecurityLevel::FullEscrow {
            interfaces[0].human_authorization = Some(true);
        }

        FaxAgentDescription {
            protocol_type: "ANP".into(),
            protocol_version: "1.0.0".into(),
            ad_type: "AgentDescription".into(),
            url: ad_url,
            name: self.name.clone(),
            did: self.did.clone(),
            owner: None,
            description: self.description.clone(),
            security_definitions: serde_json::json!({
                "didwba_sc": {
                    "scheme": "didwba",
                    "in": "header",
                    "name": "Authorization"
                }
            }),
            security: "didwba_sc".into(),
            informations,
            interfaces,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_agent_description() {
        let ad = AgentDescriptionBuilder::new(
            "did:wba:compute.io:user:alpha",
            "Compute Provider Alpha",
            "compute.io",
        )
        .description("GPU compute provider for agent workloads")
        .offer_resource(TradableResource {
            resource: ResourceAmount::new(ResourceType::Compute, 8.0, "gpu-hour")
                .with_subtype("gpu-hour"),
            quality: None,
            min_trade: Some(0.5),
            max_trade: Some(8.0),
            availability_windows: None,
        })
        .accept_type(ResourceType::LlmTokens)
        .security_level(SecurityLevel::Escrow)
        .build();

        assert_eq!(ad.protocol_type, "ANP");
        assert_eq!(ad.did, "did:wba:compute.io:user:alpha");
        assert_eq!(ad.interfaces.len(), 1);
        assert_eq!(ad.interfaces[0].protocol, "FAX");
        assert!(!ad.informations.is_empty());
        assert!(ad.informations[0].fax_resource_type.is_some());

        let json = serde_json::to_string_pretty(&ad).unwrap();
        assert!(json.contains("FAX"));
        assert!(json.contains("compute"));
    }

    #[test]
    fn test_high_security_requires_human_auth() {
        let ad = AgentDescriptionBuilder::new("did:wba:x.com:user:a", "Agent", "x.com")
            .security_level(SecurityLevel::FullEscrow)
            .build();

        assert_eq!(ad.interfaces[0].human_authorization, Some(true));
    }
}
