use crate::resource::{ResourceAmount, ResourceType};
use crate::error::{FaxError, FaxResult};

/// Resource Credit Unit — common denominator for cross-resource valuation.
/// 1 RCU ≈ cost of 1,000 tokens on a mid-tier LLM.
///
/// This is NOT a token. It's a negotiation unit for comparing heterogeneous resources.
/// Agents can disagree on rates; the blockchain doesn't enforce RCU pricing.

/// Base conversion rates to RCU. These are reference rates — agents negotiate actual prices.
pub struct RcuOracle;

impl RcuOracle {
    /// Convert a resource amount to its approximate RCU value.
    pub fn to_rcu(resource: &ResourceAmount) -> FaxResult<f64> {
        let rate = Self::base_rate(&resource.resource_type, resource.subtype.as_deref())?;
        Ok(resource.amount * rate)
    }

    /// Convert an RCU value to a resource amount.
    pub fn from_rcu(rcu: f64, resource_type: &ResourceType, subtype: Option<&str>) -> FaxResult<ResourceAmount> {
        let rate = Self::base_rate(resource_type, subtype)?;
        if rate == 0.0 {
            return Err(FaxError::RcuConversionError("zero rate".into()));
        }
        let (unit, amount) = match resource_type {
            ResourceType::Compute => ("gpu-hour", rcu / rate),
            ResourceType::LlmTokens => ("tokens", rcu / rate),
            ResourceType::KnowledgeAccess => ("query", rcu / rate),
            ResourceType::ToolAccess => ("invocation", rcu / rate),
            ResourceType::ResearchReport => ("report", rcu / rate),
            ResourceType::DataFeed => ("record", rcu / rate),
            ResourceType::ScheduleSlot => ("slot", rcu / rate),
            ResourceType::StorageQuota => ("MB-month", rcu / rate),
            ResourceType::Bandwidth => ("GB", rcu / rate),
            ResourceType::Attestation => ("attestation", rcu / rate),
            ResourceType::Custom(_) => ("unit", rcu / rate),
        };
        Ok(ResourceAmount::new(resource_type.clone(), amount, unit))
    }

    /// Base rate: how many RCU per 1 unit of this resource.
    fn base_rate(resource_type: &ResourceType, subtype: Option<&str>) -> FaxResult<f64> {
        match resource_type {
            ResourceType::Compute => match subtype {
                Some("gpu-hour") | None => Ok(50.0),
                Some("cpu-hour") => Ok(5.0),
                Some(other) => Err(FaxError::RcuConversionError(
                    format!("unknown compute subtype: {other}"),
                )),
            },
            ResourceType::LlmTokens => Ok(0.001), // 1 RCU = 1,000 tokens
            ResourceType::KnowledgeAccess => Ok(0.5),
            ResourceType::ToolAccess => Ok(0.1),
            ResourceType::ResearchReport => Ok(200.0),
            ResourceType::DataFeed => Ok(0.01),
            ResourceType::ScheduleSlot => Ok(2.0),
            ResourceType::StorageQuota => Ok(0.02),
            ResourceType::Bandwidth => Ok(0.5),
            ResourceType::Attestation => Ok(10.0),
            ResourceType::Custom(_) => Ok(1.0),
        }
    }

    /// Check if a proposed trade is roughly balanced in RCU terms.
    /// Returns the imbalance as a percentage (0.0 = perfectly balanced).
    pub fn trade_balance(
        party_a_gives: &[ResourceAmount],
        party_b_gives: &[ResourceAmount],
    ) -> FaxResult<f64> {
        let a_rcu: f64 = party_a_gives
            .iter()
            .map(|r| Self::to_rcu(r))
            .collect::<FaxResult<Vec<f64>>>()?
            .into_iter()
            .sum();

        let b_rcu: f64 = party_b_gives
            .iter()
            .map(|r| Self::to_rcu(r))
            .collect::<FaxResult<Vec<f64>>>()?
            .into_iter()
            .sum();

        if a_rcu + b_rcu == 0.0 {
            return Ok(0.0);
        }
        Ok(((a_rcu - b_rcu).abs() / (a_rcu + b_rcu)) * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_to_rcu() {
        let r = ResourceAmount::new(ResourceType::Compute, 2.0, "gpu-hour")
            .with_subtype("gpu-hour");
        let rcu = RcuOracle::to_rcu(&r).unwrap();
        assert_eq!(rcu, 100.0);
    }

    #[test]
    fn test_tokens_to_rcu() {
        let r = ResourceAmount::new(ResourceType::LlmTokens, 50000.0, "tokens");
        let rcu = RcuOracle::to_rcu(&r).unwrap();
        assert_eq!(rcu, 50.0);
    }

    #[test]
    fn test_balanced_trade() {
        let a_gives = vec![
            ResourceAmount::new(ResourceType::Compute, 2.0, "gpu-hour")
                .with_subtype("gpu-hour"),
        ];
        let b_gives = vec![
            ResourceAmount::new(ResourceType::LlmTokens, 100000.0, "tokens"),
        ];
        let imbalance = RcuOracle::trade_balance(&a_gives, &b_gives).unwrap();
        assert!(imbalance < 1.0, "trade should be roughly balanced: {imbalance}%");
    }

    #[test]
    fn test_imbalanced_trade() {
        let a_gives = vec![
            ResourceAmount::new(ResourceType::Compute, 10.0, "gpu-hour")
                .with_subtype("gpu-hour"),
        ];
        let b_gives = vec![
            ResourceAmount::new(ResourceType::LlmTokens, 1000.0, "tokens"),
        ];
        let imbalance = RcuOracle::trade_balance(&a_gives, &b_gives).unwrap();
        assert!(imbalance > 50.0, "trade should be heavily imbalanced: {imbalance}%");
    }

    #[test]
    fn test_rcu_roundtrip() {
        let original = ResourceAmount::new(ResourceType::Compute, 4.0, "gpu-hour")
            .with_subtype("gpu-hour");
        let rcu = RcuOracle::to_rcu(&original).unwrap();
        let back = RcuOracle::from_rcu(rcu, &ResourceType::Compute, Some("gpu-hour")).unwrap();
        assert!((back.amount - 4.0).abs() < 0.001);
    }
}
