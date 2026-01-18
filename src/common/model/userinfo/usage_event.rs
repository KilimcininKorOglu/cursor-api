// #[derive(serde::Serialize, Clone, Copy)]
// #[serde(rename_all = "camelCase")]
// pub struct GetAggregatedUsageEventsRequest {
//     pub team_id: i32,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub start_date: Option<i64>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub end_date: Option<i64>,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     pub user_id: Option<i32>,
// }

// #[derive(
//     serde::Serialize,
//     serde::Deserialize,
//     rkyv::Archive,
//     rkyv::Serialize,
//     rkyv::Deserialize,
//     Clone
// )]
// pub struct AggregatedUsageEvents {
//     #[serde(default)]
//     pub aggregations: Vec<get_aggregated_usage_events_response::ModelUsageAggregation>,
//     #[serde(alias = "totalInputTokens", deserialize_with = "stringify::deserialize", default)]
//     pub total_input_tokens: i64,
//     #[serde(alias = "totalOutputTokens", deserialize_with = "stringify::deserialize", default)]
//     pub total_output_tokens: i64,
//     #[serde(alias = "totalCacheWriteTokens", deserialize_with = "stringify::deserialize", default)]
//     pub total_cache_write_tokens: i64,
//     #[serde(alias = "totalCacheReadTokens", deserialize_with = "stringify::deserialize", default)]
//     pub total_cache_read_tokens: i64,
//     #[serde(alias = "totalCostCents", default)]
//     pub total_cost_cents: f64,
//     #[serde(alias = "percentOfBurstUsed", default)]
//     pub percent_of_burst_used: f64,
// }

// pub type GetAggregatedUsageEventsResponse = AggregatedUsageEvents;

// pub mod get_aggregated_usage_events_response {
//     use super::stringify;

//     #[derive(
//         serde::Serialize,
//         serde::Deserialize,
//         rkyv::Archive,
//         rkyv::Serialize,
//         rkyv::Deserialize,
//         Clone
//     )]
//     pub struct ModelUsageAggregation {
//         #[serde(alias = "modelIntent", default)]
//         pub model_intent: String,
//         #[serde(alias = "inputTokens", deserialize_with = "stringify::deserialize", default)]
//         pub input_tokens: i64,
//         #[serde(alias = "outputTokens", deserialize_with = "stringify::deserialize", default)]
//         pub output_tokens: i64,
//         #[serde(alias = "cacheWriteTokens", deserialize_with = "stringify::deserialize", default)]
//         pub cache_write_tokens: i64,
//         #[serde(alias = "cacheReadTokens", deserialize_with = "stringify::deserialize", default)]
//         pub cache_read_tokens: i64,
//         #[serde(alias = "totalCents", default)]
//         pub total_cents: f64,
//     }
// }

/// .aiserver.v1.UsageEventDisplay
#[derive(Debug, Default, Clone, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
pub struct UsageEventDisplay {
    #[serde(
        skip_serializing_if = "::proto_value::is_default",
        default,
        with = "::proto_value::stringify"
    )]
    pub timestamp: i64,
    #[serde(skip_serializing_if = "::proto_value::is_default", default)]
    pub model: ::alloc::string::String,
    #[serde(skip_serializing_if = "::proto_value::is_default", default)]
    pub kind: ::proto_value::Enum<UsageEventKind>,
    #[serde(
        rename = "customSubscriptionName",
        alias = "custom_subscription_name",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub custom_subscription_name: ::core::option::Option<::alloc::string::String>,
    #[serde(
        rename = "maxMode",
        alias = "max_mode",
        skip_serializing_if = "::proto_value::is_default",
        default
    )]
    pub max_mode: bool,
    #[serde(
        rename = "requestsCosts",
        alias = "requests_costs",
        skip_serializing_if = "::proto_value::is_default",
        default
    )]
    pub requests_costs: f32,
    #[serde(
        rename = "usageBasedCosts",
        alias = "usage_based_costs",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub usage_based_costs: ::core::option::Option<::alloc::string::String>,
    #[serde(
        rename = "isTokenBasedCall",
        alias = "is_token_based_call",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub is_token_based_call: ::core::option::Option<bool>,
    #[serde(
        rename = "tokenUsage",
        alias = "token_usage",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub token_usage: ::core::option::Option<TokenUsage>,
    #[serde(
        rename = "owningUser",
        alias = "owning_user",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub owning_user: ::core::option::Option<::alloc::string::String>,
    #[serde(
        rename = "owningTeam",
        alias = "owning_team",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub owning_team: ::core::option::Option<::alloc::string::String>,
    #[serde(
        rename = "userEmail",
        alias = "user_email",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub user_email: ::core::option::Option<::alloc::string::String>,
    #[serde(
        rename = "cursorTokenFee",
        alias = "cursor_token_fee",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub cursor_token_fee: ::core::option::Option<f32>,
    #[serde(
        rename = "isChargeable",
        alias = "is_chargeable",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub is_chargeable: ::core::option::Option<bool>,
    #[serde(
        rename = "serviceAccountName",
        alias = "service_account_name",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub service_account_name: ::core::option::Option<::alloc::string::String>,
    #[serde(
        rename = "serviceAccountId",
        alias = "service_account_id",
        skip_serializing_if = "::core::option::Option::is_none",
        default
    )]
    pub service_account_id: ::core::option::Option<::alloc::string::String>,
}

/// .aiserver.v1.TokenUsage
#[derive(Debug, Default, Clone, Copy, PartialEq, ::serde::Serialize, ::serde::Deserialize)]
pub struct TokenUsage {
    #[serde(
        rename = "inputTokens",
        alias = "input_tokens",
        skip_serializing_if = "::proto_value::is_default",
        default
    )]
    pub input_tokens: i32,
    #[serde(
        rename = "outputTokens",
        alias = "output_tokens",
        skip_serializing_if = "::proto_value::is_default",
        default
    )]
    pub output_tokens: i32,
    #[serde(
        rename = "cacheWriteTokens",
        alias = "cache_write_tokens",
        skip_serializing_if = "::proto_value::is_default",
        default
    )]
    pub cache_write_tokens: i32,
    #[serde(
        rename = "cacheReadTokens",
        alias = "cache_read_tokens",
        skip_serializing_if = "::proto_value::is_default",
        default
    )]
    pub cache_read_tokens: i32,
    #[serde(
        rename = "totalCents",
        alias = "total_cents",
        skip_serializing_if = "::proto_value::is_default",
        default
    )]
    pub total_cents: f32,
}

/// .aiserver.v1.GetFilteredUsageEventsRequest
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, ::serde::Serialize)]
pub struct GetFilteredUsageEventsRequest {
    #[serde(rename = "teamId", skip_serializing_if = "::proto_value::is_default")]
    pub team_id: i32,
    #[serde(
        rename = "startDate",
        skip_serializing_if = "::core::option::Option::is_none",
        with = "::proto_value::stringify"
    )]
    pub start_date: ::core::option::Option<i64>,
    #[serde(
        rename = "endDate",
        skip_serializing_if = "::core::option::Option::is_none",
        with = "::proto_value::stringify"
    )]
    pub end_date: ::core::option::Option<i64>,
    #[serde(rename = "userId", skip_serializing_if = "::core::option::Option::is_none")]
    pub user_id: ::core::option::Option<i32>,
    #[serde(rename = "modelId", skip_serializing_if = "::core::option::Option::is_none")]
    pub model_id: ::core::option::Option<&'static str>,
    // #[serde(skip_serializing_if = "::core::option::Option::is_none")]
    pub page: i32,
    #[serde(rename = "pageSize")]
    // #[serde(skip_serializing_if = "::core::option::Option::is_none")]
    pub page_size: i32,
}

/// .aiserver.v1.GetFilteredUsageEventsResponse
#[derive(Debug, Default, Clone, PartialEq, ::serde::Deserialize)]
pub struct GetFilteredUsageEventsResponse {
    // #[serde(rename = "totalUsageEventsCount", alias = "total_usage_events_count", default)]
    // pub total_usage_events_count: i32,
    #[serde(rename = "usageEventsDisplay", alias = "usage_events_display", default)]
    pub usage_events_display: ::alloc::vec::Vec<UsageEventDisplay>,
}

/// .aiserver.v1.UsageEventKind
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    ::serde::Serialize,
    ::serde::Deserialize,
)]
pub enum UsageEventKind {
    #[default]
    #[serde(rename = "USAGE_EVENT_KIND_UNSPECIFIED")]
    Unspecified = 0,
    #[serde(rename = "USAGE_EVENT_KIND_USAGE_BASED")]
    UsageBased = 1,
    #[serde(rename = "USAGE_EVENT_KIND_USER_API_KEY")]
    UserApiKey = 2,
    #[serde(rename = "USAGE_EVENT_KIND_INCLUDED_IN_PRO")]
    IncludedInPro = 3,
    #[serde(rename = "USAGE_EVENT_KIND_INCLUDED_IN_BUSINESS")]
    IncludedInBusiness = 4,
    #[serde(rename = "USAGE_EVENT_KIND_ERRORED_NOT_CHARGED")]
    ErroredNotCharged = 5,
    #[serde(rename = "USAGE_EVENT_KIND_ABORTED_NOT_CHARGED")]
    AbortedNotCharged = 6,
    #[serde(rename = "USAGE_EVENT_KIND_CUSTOM_SUBSCRIPTION")]
    CustomSubscription = 7,
    #[serde(rename = "USAGE_EVENT_KIND_INCLUDED_IN_PRO_PLUS")]
    IncludedInProPlus = 8,
    #[serde(rename = "USAGE_EVENT_KIND_INCLUDED_IN_ULTRA")]
    IncludedInUltra = 9,
    #[serde(rename = "USAGE_EVENT_KIND_FREE_CREDIT")]
    FreeCredit = 10,
}
impl ::core::convert::From<UsageEventKind> for i32 {
    #[inline]
    fn from(value: UsageEventKind) -> i32 { value as i32 }
}
impl ::core::convert::TryFrom<i32> for UsageEventKind {
    type Error = ();
    #[inline]
    fn try_from(value: i32) -> ::core::result::Result<Self, ()> {
        match value {
            0 => ::core::result::Result::Ok(Self::Unspecified),
            1 => ::core::result::Result::Ok(Self::UsageBased),
            2 => ::core::result::Result::Ok(Self::UserApiKey),
            3 => ::core::result::Result::Ok(Self::IncludedInPro),
            4 => ::core::result::Result::Ok(Self::IncludedInBusiness),
            5 => ::core::result::Result::Ok(Self::ErroredNotCharged),
            6 => ::core::result::Result::Ok(Self::AbortedNotCharged),
            7 => ::core::result::Result::Ok(Self::CustomSubscription),
            8 => ::core::result::Result::Ok(Self::IncludedInProPlus),
            9 => ::core::result::Result::Ok(Self::IncludedInUltra),
            10 => ::core::result::Result::Ok(Self::FreeCredit),
            _ => ::core::result::Result::Err(()),
        }
    }
}
