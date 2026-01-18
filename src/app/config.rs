use alloc::borrow::Cow;

use super::{constant::AUTHORIZATION_BEARER_PREFIX, lazy::AUTH_TOKEN, model::AppConfig};
use crate::common::model::{
    ApiStatus, GenericError,
    config::{ConfigData, ConfigResponse, ConfigUpdateRequest},
};
use axum::{
    Json,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
};

// 定义处理更新操作的宏
macro_rules! handle_updates {
    ($request:expr, $($field:ident => $update_fn:expr),* $(,)?) => {
        $(
            if let Some(value) = $request.$field {
                $update_fn(value);
            }
        )*
    };
}

// 定义处理重置操作的宏
macro_rules! handle_resets {
    ($request:expr, $($field:ident => $reset_fn:expr),* $(,)?) => {
        $(
            if $request.$field.is_some() {
                $reset_fn();
            }
        )*
    };
}

pub async fn handle_config_update(
    headers: HeaderMap,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<ConfigResponse>, (StatusCode, Json<GenericError>)> {
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(GenericError {
                status: ApiStatus::Error,
                code: Some(StatusCode::UNAUTHORIZED),
                error: Some(Cow::Borrowed("未提供认证令牌")),
                message: None,
            }),
        ))?;

    if auth_header != *AUTH_TOKEN {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(GenericError {
                status: ApiStatus::Error,
                code: Some(StatusCode::UNAUTHORIZED),
                error: Some(Cow::Borrowed("无效的认证令牌")),
                message: None,
            }),
        ));
    }

    match request.action.as_str() {
        "get" => Ok(Json(ConfigResponse {
            status: ApiStatus::Success,
            data: Some(ConfigData {
                vision_ability: AppConfig::get_vision_ability(),
                enable_slow_pool: AppConfig::get_slow_pool(),
                enable_long_context: AppConfig::get_long_context(),
                usage_check_models: AppConfig::get_usage_check(),
                dynamic_key_secret: AppConfig::get_dynamic_key_secret(),
                share_token: AppConfig::get_share_token(),
                include_web_references: AppConfig::get_web_refs(),
                fetch_raw_models: AppConfig::get_fetch_models(),
            }),
            message: None,
        })),

        "update" => {
            handle_updates!(request,
                vision_ability => AppConfig::update_vision_ability,
                enable_slow_pool => AppConfig::update_slow_pool,
                enable_long_context => AppConfig::update_long_context,
                usage_check_models => AppConfig::update_usage_check,
                dynamic_key_secret => AppConfig::update_dynamic_key_secret,
                share_token => AppConfig::update_share_token,
                include_web_references => AppConfig::update_web_refs,
                fetch_raw_models => AppConfig::update_fetch_models,
            );

            Ok(Json(ConfigResponse {
                status: ApiStatus::Success,
                data: None,
                message: Some(Cow::Borrowed("配置已更新")),
            }))
        }

        "reset" => {
            handle_resets!(request,
                vision_ability => AppConfig::reset_vision_ability,
                enable_slow_pool => AppConfig::reset_slow_pool,
                enable_long_context => AppConfig::reset_long_context,
                usage_check_models => AppConfig::reset_usage_check,
                dynamic_key_secret => AppConfig::reset_dynamic_key_secret,
                share_token => AppConfig::reset_share_token,
                include_web_references => AppConfig::reset_web_refs,
                fetch_raw_models => AppConfig::reset_fetch_models,
            );

            Ok(Json(ConfigResponse {
                status: ApiStatus::Success,
                data: None,
                message: Some(Cow::Borrowed("配置已重置")),
            }))
        }

        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: Some(StatusCode::BAD_REQUEST),
                error: Some(Cow::Borrowed("无效的操作类型")),
                message: None,
            }),
        )),
    }
}
