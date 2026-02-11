use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct UsageSummary {
    /// 订阅计划的UseCase（包含的额度）
    pub plan: Option<PlanUsage>,
    /// 按需UseCase（超出包含额度后的付费Use）
    #[serde(alias = "onDemand", skip_serializing_if = "Option::is_none")]
    pub on_demand: Option<OnDemandUsage>,
}

/// 个人用户的UseCase
pub type IndividualUsage = UsageSummary;

/// 团队级别的UseCase
pub type TeamUsage = UsageSummary;

/// 订阅计划的UseCase
///
/// 包含计划自带的API usage额度，以API价格计费
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct PlanUsage {
    pub enabled: bool,
    /// AlreadyUseAmount（May是花费单位OrRequest计Amount单位）
    pub used: i32,
    /// 配额上限（Current计费周期内的总限额）
    pub limit: i32,
    /// 剩余可用Amount (= limit - used)
    pub remaining: i32,
    /// 配额来源细分
    #[serde(default)]
    pub breakdown: UsageBreakdown,
}

/// 配额来源细分
///
/// - `included`: 计划包含的基础配额（如Pro的$20对应的Amount）
/// - `bonus`: 额外赠送的bonus capacity（Animated发放）
/// - `total`: included + bonus（总承诺配额）
///
/// 注意：`total`May小于Or等于`PlanUsage.limit`，其中：
/// - `limit`是账户的总配额上限
/// - `breakdown`记录Already发放/Statistics的配额细分
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct UsageBreakdown {
    /// 基础包含配额
    pub included: i32,
    /// 额外赠送配额（"work hard to grant additional bonus capacity"）
    pub bonus: i32,
    /// 总计 = included + bonus
    pub total: i32,
}

/// 按需UseCase
///
/// 当用户超出计划包含的配额后，可启用on-demand付费Use
/// 按相同的API价格计费，无质AmountOr速度降级
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct OnDemandUsage {
    /// Whether启用按需计费
    pub enabled: bool,
    /// AlreadyUse的按需配额
    pub used: i32,
    /// 按需配额上限（None表示无LimitOr未设置）
    pub limit: Option<i32>,
    /// 剩余按需配额
    pub remaining: Option<i32>,
}
