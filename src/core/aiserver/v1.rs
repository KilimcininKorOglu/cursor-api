use core::num::NonZeroU16;

// Include the generated Protobuf code
// include!(concat!(env!("OUT_DIR"), "/aiserver.v1.rs"));
include!("v1/lite.rs");

impl ErrorDetails {
    /// 将Error转换为相应的 HTTP 状态码。
    ///
    /// 此方法根据Error的性质，将内部Error类型映射到标准的 HTTP 状态码，
    /// 遵循 RESTful API 最佳实践。
    ///
    /// 返回值：
    ///   - u16: 与Error对应的 HTTP 状态码。
    pub fn status_code(error: i32) -> NonZeroU16 {
        use error_details::Error;
        let code = match Error::try_from(error) {
            Ok(error) => match error {
                // 400 - Bad Request: 客户端Error，RequestFormatError或无效
                Error::BadRequest
                | Error::BadModelName
                | Error::SlashEditFileTooLong
                | Error::FileUnsupported
                | Error::ClaudeImageTooLarge
                | Error::ConversationTooLong => 400,

                // 401 - Unauthorized: 身份验证相关Error
                Error::BadApiKey
                | Error::BadUserApiKey
                | Error::InvalidAuthId
                | Error::AuthTokenNotFound
                | Error::AuthTokenExpired
                | Error::Unauthorized
                | Error::GithubNoUserCredentials => 401,

                // 402 - Payment Required: 需要付费
                Error::UsagePricingRequired | Error::UsagePricingRequiredChangeable => 402,

                // 403 - Forbidden: 权限相关Error
                Error::NotLoggedIn
                | Error::NotHighEnoughPermissions
                | Error::AgentRequiresLogin
                | Error::ProUserOnly
                | Error::TaskNoPermissions
                | Error::GithubUserNoAccess
                | Error::GithubAppNoAccess
                | Error::HooksBlocked => 403,

                // 404 - Not Found: 资源未找到Error
                Error::NotFound
                | Error::UserNotFound
                | Error::TaskUuidNotFound
                | Error::AgentEngineNotFound
                | Error::GitgraphNotFound
                | Error::FileNotFound => 404,

                // 409 - Conflict: 资源状态冲突
                Error::GithubMultipleOwners => 409,

                // 410 - Gone: 资源不再可用
                Error::Deprecated | Error::OutdatedClient => 410,

                // 422 - Unprocessable Entity: Request有效但无法处理
                Error::ApiKeyNotSupported => 422,

                // 429 - Too Many Requests: 限流相关Error
                Error::FreeUserRateLimitExceeded
                | Error::ProUserRateLimitExceeded
                | Error::OpenaiRateLimitExceeded
                | Error::OpenaiAccountLimitExceeded
                | Error::GenericRateLimitExceeded
                | Error::Gpt4VisionPreviewRateLimit
                | Error::ApiKeyRateLimit
                | Error::RateLimited
                | Error::RateLimitedChangeable => 429,

                // 499 - Client Closed Request: 客户端关闭Request（非标准但常用）
                Error::UserAbortedRequest => 499,

                // 503 - Service Unavailable: 服务器因过载或维护暂时不可用
                Error::FreeUserUsageLimit
                | Error::ProUserUsageLimit
                | Error::ResourceExhausted
                | Error::MaxTokens => 503,

                // 504 - Gateway Timeout: 网关超时
                Error::Timeout => 504,

                // 533 - Upstream Failure: 上游服务报告Failed（非标准）
                Error::Unspecified
                | Error::Openai
                | Error::CustomMessage
                | Error::Debounced
                | Error::RepositoryServiceRepositoryIsNotInitialized
                | Error::Custom => 533,
            },
            // 未在上游枚举中定义的Error被视为真正的内部服务器Error
            Err(_) => 500,
        };
        unsafe { NonZeroU16::new_unchecked(code) }
    }

    /// 返回Error类型的 snake_case 字符串表示。
    ///
    /// 此方法将Error变体映射到其 snake_case 字符串名称，
    /// 用于日志记录、Debug或 API Response。
    ///
    /// 返回值：
    ///   - &'static str: Error类型的 snake_case 名称。
    pub fn r#type(error: i32) -> &'static str {
        use error_details::Error;
        match Error::try_from(error) {
            Ok(error) => match error {
                Error::Unspecified => "unspecified",
                Error::BadApiKey => "bad_api_key",
                Error::BadUserApiKey => "bad_user_api_key",
                Error::NotLoggedIn => "not_logged_in",
                Error::InvalidAuthId => "invalid_auth_id",
                Error::NotHighEnoughPermissions => "not_high_enough_permissions",
                Error::AgentRequiresLogin => "agent_requires_login",
                Error::BadModelName => "bad_model_name",
                Error::NotFound => "not_found",
                Error::Deprecated => "deprecated",
                Error::UserNotFound => "user_not_found",
                Error::FreeUserRateLimitExceeded => "free_user_rate_limit_exceeded",
                Error::ProUserRateLimitExceeded => "pro_user_rate_limit_exceeded",
                Error::FreeUserUsageLimit => "free_user_usage_limit",
                Error::ProUserUsageLimit => "pro_user_usage_limit",
                Error::ResourceExhausted => "resource_exhausted",
                Error::AuthTokenNotFound => "auth_token_not_found",
                Error::AuthTokenExpired => "auth_token_expired",
                Error::Openai => "openai",
                Error::OpenaiRateLimitExceeded => "openai_rate_limit_exceeded",
                Error::OpenaiAccountLimitExceeded => "openai_account_limit_exceeded",
                Error::TaskUuidNotFound => "task_uuid_not_found",
                Error::TaskNoPermissions => "task_no_permissions",
                Error::AgentEngineNotFound => "agent_engine_not_found",
                Error::MaxTokens => "max_tokens",
                Error::ProUserOnly => "pro_user_only",
                Error::ApiKeyNotSupported => "api_key_not_supported",
                Error::UserAbortedRequest => "user_aborted_request",
                Error::Timeout => "timeout",
                Error::GenericRateLimitExceeded => "generic_rate_limit_exceeded",
                Error::SlashEditFileTooLong => "slash_edit_file_too_long",
                Error::FileUnsupported => "file_unsupported",
                Error::Gpt4VisionPreviewRateLimit => "gpt4_vision_preview_rate_limit",
                Error::CustomMessage => "custom_message",
                Error::OutdatedClient => "outdated_client",
                Error::ClaudeImageTooLarge => "claude_image_too_large",
                Error::GitgraphNotFound => "gitgraph_not_found",
                Error::FileNotFound => "file_not_found",
                Error::ApiKeyRateLimit => "api_key_rate_limit",
                Error::Debounced => "debounced",
                Error::BadRequest => "bad_request",
                Error::RepositoryServiceRepositoryIsNotInitialized => {
                    "repository_service_repository_is_not_initialized"
                }
                Error::Unauthorized => "unauthorized",
                Error::ConversationTooLong => "conversation_too_long",
                Error::UsagePricingRequired => "usage_pricing_required",
                Error::UsagePricingRequiredChangeable => "usage_pricing_required_changeable",
                Error::GithubNoUserCredentials => "github_no_user_credentials",
                Error::GithubUserNoAccess => "github_user_no_access",
                Error::GithubAppNoAccess => "github_app_no_access",
                Error::GithubMultipleOwners => "github_multiple_owners",
                Error::RateLimited => "rate_limited",
                Error::RateLimitedChangeable => "rate_limited_changeable",
                Error::Custom => "custom",
                Error::HooksBlocked => "hooks_blocked",
            },
            Err(_) => crate::app::constant::UNKNOWN, // 未知Error类型的默认值
        }
    }
}

impl CustomErrorDetails {
    #[inline]
    pub fn add(&mut self, rhs: Self) {
        #[inline(always)]
        fn add_string(a: &mut String, b: String) {
            a.reserve(b.len() + 1);
            a.push('&');
            a.push_str(&b);
        }
        add_string(&mut self.title, rhs.title);
        add_string(&mut self.detail, rhs.detail);
        // self.buttons.extend(rhs.buttons);
        self.additional_info.extend(rhs.additional_info);
    }
}

impl From<conversation_message::Thinking> for super::super::stream::decoder::Thinking {
    #[inline]
    fn from(thinking: conversation_message::Thinking) -> Self {
        if !thinking.text.is_empty() {
            Self::Text(thinking.text)
        } else if !thinking.signature.is_empty() {
            Self::Signature(thinking.signature)
        } else if !thinking.redacted_thinking.is_empty() {
            Self::RedactedThinking(thinking.redacted_thinking)
        } else {
            Self::Text(thinking.text)
        }
    }
}

impl TryFrom<image::DynamicImage> for image_proto::Dimension {
    type Error = core::num::TryFromIntError;
    #[inline]
    fn try_from(img: image::DynamicImage) -> Result<Self, Self::Error> {
        Ok(Self { width: img.width().try_into()?, height: img.height().try_into()? })
    }
}

impl AvailableModelsResponse {
    /// 根据 `AvailableModel` 的关键字段（`name`、`client_display_name`、`server_model_name`）
    /// 判断两个Response是否相等。
    ///
    /// # 参数
    ///
    /// * `other` - 要比较的另一个 `AvailableModelsResponse` 实例。
    pub fn is_subset_equal(&self, other: &Self) -> bool {
        if self.models.len() != other.models.len() {
            return false;
        }

        self.models.iter().zip(other.models.iter()).all(|(a, b)| {
            a.name == b.name
                && a.client_display_name == b.client_display_name
                && a.server_model_name == b.server_model_name
        })
    }
}
