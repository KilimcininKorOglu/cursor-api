use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct UsageSummary {
    /// Subscription plan usage (included quota)
    pub plan: Option<PlanUsage>,
    /// On-demand usage (paid usage after exceeding included quota)
    #[serde(alias = "onDemand", skip_serializing_if = "Option::is_none")]
    pub on_demand: Option<OnDemandUsage>,
}

/// Individual user usage
pub type IndividualUsage = UsageSummary;

/// Team-level usage
pub type TeamUsage = UsageSummary;

/// Subscription plan usage
///
/// Contains API usage quota included with the plan, billed at API prices
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct PlanUsage {
    pub enabled: bool,
    /// Already used amount (may be spending units or request count units)
    pub used: i32,
    /// Quota limit (total limit for current billing period)
    pub limit: i32,
    /// Remaining available amount (= limit - used)
    pub remaining: i32,
    /// Quota source breakdown
    #[serde(default)]
    pub breakdown: UsageBreakdown,
}

/// Quota source breakdown
///
/// - `included`: Base quota included with plan (e.g., Pro's $20 corresponding amount)
/// - `bonus`: Additional bonus capacity granted (distributed periodically)
/// - `total`: included + bonus (total committed quota)
///
/// Note: `total` may be less than or equal to `PlanUsage.limit`, where:
/// - `limit` is the account's total quota ceiling
/// - `breakdown` records already distributed/calculated quota breakdown
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct UsageBreakdown {
    /// Base included quota
    pub included: i32,
    /// Additional bonus quota ("work hard to grant additional bonus capacity")
    pub bonus: i32,
    /// Total = included + bonus
    pub total: i32,
}

/// On-demand usage
///
/// When users exceed the quota included with their plan, they can enable on-demand paid usage
/// Billed at the same API price, no quality or speed degradation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct OnDemandUsage {
    /// Whether on-demand billing is enabled
    pub enabled: bool,
    /// Already used on-demand quota
    pub used: i32,
    /// On-demand quota limit (None means no limit or not set)
    pub limit: Option<i32>,
    /// Remaining on-demand quota
    pub remaining: Option<i32>,
}
