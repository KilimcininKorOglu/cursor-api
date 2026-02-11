use crate::{
    app::{
        constant::UNNAMED,
        model::{
            AppState, Checksum, CommonResponse, ExtToken, GcppHost, Hash, RawToken, Token,
            TokenError, TokenHealth, TokenInfo, TokenManager, TokenStatus, TokensAddRequest,
            TokensAddResponse, TokensAliasSetRequest, TokensDeleteRequest, TokensDeleteResponse,
            TokensGetResponse, TokensMergeRequest, TokensProxySetRequest, TokensStatusSetRequest,
            TokensTimezoneSetRequest, TokensUpdateRequest,
        },
    },
    common::model::{ApiStatus, GenericError},
};
use alloc::{borrow::Cow, sync::Arc};
use axum::{Json, extract::State};
use core::str::FromStr as _;
use http::StatusCode;
use interned::ArcStr;

type HashSet<K> = hashbrown::HashSet<K, ahash::RandomState>;

crate::define_typed_constants! {
    &'static str => {
        SET_SUCCESS = "Set successfully",
        SET_FAILURE_COUNT = "tokens failed to set",
        UPDATE_SUCCESS = "Updated",
        UPDATE_FAILURE_COUNT = "tokens failed to update",
        ERROR_SAVE_TOKEN_DATA = "Failed to save token data",
        MESSAGE_SAVE_TOKEN_DATA_FAILED = "Failed to save token data",
        ERROR_NO_TOKENS_PROVIDED = "No tokens provided",
        MESSAGE_NO_TOKENS_PROVIDED = "No tokens provided",
        ERROR_SAVE_TOKEN_PROFILES = "Failed to save token profiles",
        ERROR_SAVE_TOKENS = "Failed to save tokens",
        ERROR_SAVE_TOKEN_STATUSES = "Failed to save token statuses",
        ERROR_SAVE_TOKEN_ALIASES = "Failed to save token aliases",
        ERROR_SAVE_TOKEN_PROXIES = "Failed to save token proxies",
        ERROR_SAVE_TOKEN_TIMEZONES = "Failed to save token timezones",
        MESSAGE_SAVE_TOKEN_PROFILE_FAILED = "Failed to save token profile data",
        MESSAGE_SAVE_TOKEN_CONFIG_VERSION_FAILED = "Failed to save token config version data",
        MESSAGE_SAVE_TOKEN_STATUS_FAILED = "Failed to save token status data",
        MESSAGE_SAVE_TOKEN_PROXY_FAILED = "Failed to save token proxy data",
        MESSAGE_SAVE_TOKEN_TIMEZONE_FAILED = "Failed to save token timezone data",
    }
}

pub async fn handle_get_tokens(State(state): State<Arc<AppState>>) -> Json<TokensGetResponse> {
    let tokens = state.token_manager_read().await.list();

    Json(TokensGetResponse { tokens })
}

pub async fn handle_set_tokens(
    State(state): State<Arc<AppState>>,
    Json(tokens): Json<TokensUpdateRequest>,
) -> Result<Json<TokensAddResponse>, StatusCode> {
    // Get write lock and update token manager
    let mut token_manager = state.token_manager_write().await;
    *token_manager = TokenManager::new(tokens.len());
    for (alias, token_info) in tokens {
        let _ = token_manager.add(token_info, alias);
    }
    let tokens_count = token_manager.tokens().len();

    // Save to file
    token_manager.save().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(TokensAddResponse {
        tokens_count,
        message: "Token files have been updated and reloaded",
    }))
}

pub async fn handle_add_tokens(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TokensAddRequest>,
) -> Result<Json<TokensAddResponse>, (StatusCode, Json<GenericError>)> {
    // Get write lock on token manager
    let mut token_manager = state.token_manager_write().await;

    // Create set of existing tokens
    let existing_tokens: HashSet<_> = token_manager
        .tokens()
        .iter()
        .flatten()
        .map(|info| info.bundle.primary_token.as_str())
        .collect();

    // Process new tokens
    let mut new_tokens = Vec::with_capacity(request.tokens.len());
    for token_info in request.tokens {
        if !existing_tokens.contains(token_info.token.as_str())
            && let Ok(raw) = <RawToken as ::core::str::FromStr>::from_str(&token_info.token)
        {
            new_tokens.push((
                TokenInfo {
                    bundle: ExtToken {
                        primary_token: Token::new(raw, Some(token_info.token)),
                        secondary_token: None,
                        checksum: token_info
                            .checksum
                            .as_deref()
                            .map(Checksum::repair)
                            .unwrap_or_default(),
                        client_key: token_info
                            .client_key
                            .and_then(|s| Hash::from_str(&s).ok())
                            .unwrap_or_else(Hash::random),
                        session_id: token_info
                            .session_id
                            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
                            .unwrap_or_else(uuid::Uuid::new_v4),
                        config_version: token_info
                            .config_version
                            .and_then(|s| uuid::Uuid::parse_str(&s).ok()),
                        proxy: token_info.proxy.map(ArcStr::new),
                        timezone: token_info
                            .timezone
                            .and_then(|s| chrono_tz::Tz::from_str(&s).ok()),
                        gcpp_host: token_info.gcpp_host.and_then(|s| GcppHost::from_str(&s)),
                    },
                    status: TokenStatus { enabled: request.enabled, health: TokenHealth::new() },
                    usage: None,
                    user: None,
                    stripe: None,
                    sessions: vec![],
                },
                token_info
                    .alias
                    .filter(|s| s.split_whitespace().next().is_some())
                    .map(Cow::Owned)
                    .unwrap_or(Cow::Borrowed(UNNAMED)),
            ));
        }
    }

    // Only proceed if there are new tokens
    if !new_tokens.is_empty() {
        // Add new tokens
        for (token_info, alias) in new_tokens {
            let _ = token_manager.add(token_info, alias);
        }
        let tokens_count = token_manager.tokens().len();

        // Save to file
        token_manager.save().await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GenericError {
                    status: ApiStatus::Error,
                    code: None,
                    error: Some(Cow::Borrowed(ERROR_SAVE_TOKEN_DATA)),
                    message: Some(Cow::Borrowed(MESSAGE_SAVE_TOKEN_DATA_FAILED)),
                }),
            )
        })?;

        Ok(Json(TokensAddResponse {
            tokens_count,
            message: "New tokens have been added and reloaded",
        }))
    } else {
        // If no new tokens, return current state
        let tokens_count = token_manager.tokens().len();

        Ok(Json(TokensAddResponse { tokens_count, message: "No new tokens were added" }))
    }
}

pub async fn handle_delete_tokens(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TokensDeleteRequest>,
) -> Result<Json<TokensDeleteResponse>, (StatusCode, Json<GenericError>)> {
    let mut token_manager = state.token_manager_write().await;

    // Complete deletion and failure recording in one pass
    let (has_updates, failed_tokens) = {
        let mut has_updates = false;
        let mut failed_tokens = if request.include_failed_tokens { Some(Vec::new()) } else { None };

        for alias in request.aliases {
            match token_manager.alias_map().get(alias.as_str()) {
                Some(&id) => {
                    let _ = token_manager.remove(id);
                    has_updates = true;
                }
                None => {
                    if let Some(ref mut failed) = failed_tokens {
                        failed.push(alias);
                    }
                }
            }
        }

        (has_updates, failed_tokens)
    };

    // Save if there are updates
    if has_updates {
        token_manager.save().await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(GenericError {
                    status: ApiStatus::Success,
                    code: None,
                    error: Some(Cow::Borrowed(ERROR_SAVE_TOKEN_DATA)),
                    message: Some(Cow::Borrowed(MESSAGE_SAVE_TOKEN_DATA_FAILED)),
                }),
            )
        })?;
    }

    Ok(Json(TokensDeleteResponse { status: ApiStatus::Success, failed_tokens }))
}

pub async fn handle_update_tokens_profile(
    State(state): State<Arc<AppState>>,
    Json(aliases): Json<HashSet<String>>,
) -> Result<Json<CommonResponse>, (StatusCode, Json<GenericError>)> {
    // Validate request
    if aliases.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_NO_TOKENS_PROVIDED)),
                message: Some(Cow::Borrowed(MESSAGE_NO_TOKENS_PROVIDED)),
            }),
        ));
    }

    // Get current token_manager
    let mut token_manager = state.token_manager_write().await;

    // Batch set tokens profile
    let mut updated_count = 0u32;
    let mut failed_count = 0u32;

    let mut alias_updaters: Vec<(usize, String)> = Vec::with_capacity(aliases.len());

    for alias in &aliases {
        // Verify token exists in token_manager
        if let Some(id) = token_manager.alias_map().get(alias.as_str()).copied() {
            let alias_is_unnamed = unsafe {
                token_manager
                    .id_to_alias()
                    .get_unchecked(id)
                    .as_ref()
                    .unwrap_unchecked()
                    .is_unnamed()
            };
            let token_info = unsafe { token_manager.tokens_mut().get_unchecked_mut(id) };

            // Get profile
            let (usage, stripe, user, sessions) = crate::common::utils::get_token_profile(
                token_info.bundle.get_client(),
                token_info.bundle.as_unext(),
                true,
                true,
            )
            .await;

            // Set profile
            if alias_is_unnamed
                && let Some(ref user) = user
                && let Some(alias) = user.alias()
            {
                // Safety: capacity == aliases.len && token_info.len <= aliases.len
                unsafe {
                    let len = alias_updaters.len();
                    let end = alias_updaters.as_mut_ptr().add(len);
                    ::core::ptr::write(end, (id, alias.clone()));
                    alias_updaters.set_len(len + 1);
                }
            }
            token_info.usage = usage;
            token_info.user = user;
            token_info.stripe = stripe;
            if let Some(sessions) = sessions {
                token_info.sessions = sessions;
            }
            updated_count += 1;
        } else {
            failed_count += 1;
        }
    }

    for (id, alias) in alias_updaters {
        let _ = token_manager.set_alias(id, alias);
    }

    // Save changes
    if updated_count > 0 && token_manager.save().await.is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_SAVE_TOKEN_PROFILES)),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_TOKEN_PROFILE_FAILED)),
            }),
        ));
    }

    Ok(Json(CommonResponse {
        status: ApiStatus::Success,
        message: Cow::Owned(
            [
                UPDATE_SUCCESS,
                itoa::Buffer::new().format(updated_count),
                " token profiles, ",
                itoa::Buffer::new().format(failed_count),
                UPDATE_FAILURE_COUNT,
            ]
            .concat(),
        ),
    }))
}

pub async fn handle_update_tokens_config_version(
    State(state): State<Arc<AppState>>,
    Json(aliases): Json<HashSet<String>>,
) -> Result<Json<CommonResponse>, (StatusCode, Json<GenericError>)> {
    if aliases.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_NO_TOKENS_PROVIDED)),
                message: Some(Cow::Borrowed(MESSAGE_NO_TOKENS_PROVIDED)),
            }),
        ));
    }

    let mut token_manager = state.token_manager_write().await;

    let mut updated_count = 0u32;
    let mut failed_count = 0u32;
    let mut short_token_count = 0u32;

    for alias in aliases {
        if let Some(info) = token_manager
            .alias_map()
            .get(alias.as_str())
            .copied()
            .map(|id| unsafe { token_manager.tokens_mut().get_unchecked_mut(id) })
        {
            if info.bundle.primary_token.is_web() {
                short_token_count += 1;
                failed_count += 1;
            } else if let Some(config_version) = {
                crate::common::utils::get_server_config(
                    info.bundle.clone_without_config_version(),
                    true,
                )
                .await
            } {
                info.bundle.config_version = Some(config_version);
                updated_count += 1;
            } else {
                failed_count += 1;
            }
        } else {
            failed_count += 1;
        }
    }

    // Save changes
    if updated_count > 0 && token_manager.save().await.is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_SAVE_TOKEN_PROFILES)),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_TOKEN_CONFIG_VERSION_FAILED)),
            }),
        ));
    }

    let message = if short_token_count > 0 {
        [
            UPDATE_SUCCESS,
            itoa::Buffer::new().format(updated_count),
            " token config versions; ",
            itoa::Buffer::new().format(failed_count),
            " token updates failed, of which ",
            itoa::Buffer::new().format(short_token_count),
            " tokens are non-session tokens",
        ]
        .concat()
    } else {
        [
            UPDATE_SUCCESS,
            itoa::Buffer::new().format(updated_count),
            " token config versions; ",
            itoa::Buffer::new().format(failed_count),
            UPDATE_FAILURE_COUNT,
        ]
        .concat()
    };

    Ok(Json(CommonResponse { status: ApiStatus::Success, message: Cow::Owned(message) }))
}

pub async fn handle_refresh_tokens(
    State(state): State<Arc<AppState>>,
    Json(aliases): Json<HashSet<String>>,
) -> Result<Json<CommonResponse>, (StatusCode, Json<GenericError>)> {
    if aliases.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_NO_TOKENS_PROVIDED)),
                message: Some(Cow::Borrowed(MESSAGE_NO_TOKENS_PROVIDED)),
            }),
        ));
    }

    let mut token_manager = state.token_manager_write().await;

    let mut updated_count = 0u32;
    let mut failed_count = 0u32;

    for alias in aliases {
        if let Some(writer) = token_manager
            .alias_map()
            .get(alias.as_str())
            .copied()
            .map(|id| unsafe { token_manager.tokens_mut().into_token_writer(id) })
            && crate::common::utils::get_new_token(writer, true).await
        {
            updated_count += 1;
        } else {
            failed_count += 1;
        }
    }

    // 保存更改
    if updated_count > 0 && token_manager.save().await.is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_SAVE_TOKENS)),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_TOKEN_DATA_FAILED)),
            }),
        ));
    }

    Ok(Json(CommonResponse {
        status: ApiStatus::Success,
        message: Cow::Owned(
            [
                "Refreshed ",
                itoa::Buffer::new().format(updated_count),
                " tokens, ",
                itoa::Buffer::new().format(failed_count),
                " token refresh failed",
            ]
            .concat(),
        ),
    }))
}

pub async fn handle_set_tokens_status(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TokensStatusSetRequest>,
) -> Result<Json<CommonResponse>, (StatusCode, Json<GenericError>)> {
    // Validate request
    if request.aliases.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_NO_TOKENS_PROVIDED)),
                message: Some(Cow::Borrowed(MESSAGE_NO_TOKENS_PROVIDED)),
            }),
        ));
    }

    // Get current token_manager
    let mut token_manager = state.token_manager_write().await;

    // Batch set tokens profile
    let mut updated_count = 0u32;
    let mut failed_count = 0u32;

    for alias in request.aliases {
        // Verify token exists in token_manager
        if let Some(info) = token_manager
            .alias_map()
            .get(alias.as_str())
            .copied()
            .map(|id| unsafe { token_manager.tokens_mut().get_unchecked_mut(id) })
        {
            info.status.enabled = request.enabled;
            updated_count += 1;
        } else {
            failed_count += 1;
        }
    }

    // Save changes
    if updated_count > 0 && token_manager.save().await.is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_SAVE_TOKEN_STATUSES)),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_TOKEN_STATUS_FAILED)),
            }),
        ));
    }

    Ok(Json(CommonResponse {
        status: ApiStatus::Success,
        message: Cow::Owned(
            [
                SET_SUCCESS,
                itoa::Buffer::new().format(updated_count),
                " token statuses, ",
                itoa::Buffer::new().format(failed_count),
                SET_FAILURE_COUNT,
            ]
            .concat(),
        ),
    }))
}

pub async fn handle_set_tokens_alias(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TokensAliasSetRequest>,
) -> Result<Json<CommonResponse>, (StatusCode, Json<GenericError>)> {
    // Validate request
    if request.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_NO_TOKENS_PROVIDED)),
                message: Some(Cow::Borrowed(MESSAGE_NO_TOKENS_PROVIDED)),
            }),
        ));
    }

    let mut token_manager = state.token_manager_write().await;
    let mut updated_count = 0u32;
    let mut failed_count = 0u32;

    for (old_alias, new_alias) in request {
        // Find token ID by old alias
        match token_manager.alias_map().get(old_alias.as_str()).copied() {
            Some(token_id) => {
                // Use set_alias method to update alias
                match token_manager.set_alias(token_id, new_alias) {
                    Ok(()) => updated_count += 1,
                    Err(TokenError::AliasExists) => {
                        // New alias already exists
                        failed_count += 1;
                    }
                    Err(TokenError::InvalidId) => {
                        // Should not happen in theory, as ID is from alias_map
                        failed_count += 1;
                    }
                }
            }
            None => {
                // Cannot find corresponding old alias
                failed_count += 1;
            }
        }
    }

    // Save changes
    if updated_count > 0
        && let Err(e) = token_manager.save().await
    {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_SAVE_TOKEN_ALIASES)),
                message: Some(Cow::Owned(e.to_string())),
            }),
        ));
    }

    Ok(Json(CommonResponse {
        status: ApiStatus::Success,
        message: Cow::Owned(
            [
                SET_SUCCESS,
                itoa::Buffer::new().format(updated_count),
                " token aliases, ",
                itoa::Buffer::new().format(failed_count),
                SET_FAILURE_COUNT,
            ]
            .concat(),
        ),
    }))
}

pub async fn handle_set_tokens_proxy(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TokensProxySetRequest>,
) -> Result<Json<CommonResponse>, (StatusCode, Json<GenericError>)> {
    // Validate request
    if request.aliases.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_NO_TOKENS_PROVIDED)),
                message: Some(Cow::Borrowed(MESSAGE_NO_TOKENS_PROVIDED)),
            }),
        ));
    }

    // Get current token_manager
    let mut token_manager = state.token_manager_write().await;

    // Batch set tokens proxy
    let mut updated_count = 0u32;
    let mut failed_count = 0u32;

    for alias in request.aliases {
        // Verify token exists in token_manager
        if let Some(info) = token_manager
            .alias_map()
            .get(alias.as_str())
            .copied()
            .map(|id| unsafe { token_manager.tokens_mut().get_unchecked_mut(id) })
        {
            info.bundle.proxy = request.proxy.clone();
            updated_count += 1;
        } else {
            failed_count += 1;
        }
    }

    // Save changes
    if updated_count > 0 && token_manager.save().await.is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_SAVE_TOKEN_PROXIES)),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_TOKEN_PROXY_FAILED)),
            }),
        ));
    }

    Ok(Json(CommonResponse {
        status: ApiStatus::Success,
        message: Cow::Owned(
            [
                SET_SUCCESS,
                itoa::Buffer::new().format(updated_count),
                " token proxies, ",
                itoa::Buffer::new().format(failed_count),
                SET_FAILURE_COUNT,
            ]
            .concat(),
        ),
    }))
}

pub async fn handle_set_tokens_timezone(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TokensTimezoneSetRequest>,
) -> Result<Json<CommonResponse>, (StatusCode, Json<GenericError>)> {
    // Validate request
    if request.aliases.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_NO_TOKENS_PROVIDED)),
                message: Some(Cow::Borrowed(MESSAGE_NO_TOKENS_PROVIDED)),
            }),
        ));
    }

    // Get current token_manager
    let mut token_manager = state.token_manager_write().await;

    // Batch set tokens timezone
    let mut updated_count = 0u32;
    let mut failed_count = 0u32;

    for alias in request.aliases {
        // Verify token exists in token_manager
        if let Some(info) = token_manager
            .alias_map()
            .get(alias.as_str())
            .copied()
            .map(|id| unsafe { token_manager.tokens_mut().get_unchecked_mut(id) })
        {
            info.bundle.timezone = request.timezone;
            updated_count += 1;
        } else {
            failed_count += 1;
        }
    }

    // Save changes
    if updated_count > 0 && token_manager.save().await.is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_SAVE_TOKEN_TIMEZONES)),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_TOKEN_TIMEZONE_FAILED)),
            }),
        ));
    }

    Ok(Json(CommonResponse {
        status: ApiStatus::Success,
        message: Cow::Owned(
            [
                SET_SUCCESS,
                itoa::Buffer::new().format(updated_count),
                " token timezones, ",
                itoa::Buffer::new().format(failed_count),
                SET_FAILURE_COUNT,
            ]
            .concat(),
        ),
    }))
}

pub async fn handle_merge_tokens(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TokensMergeRequest>,
) -> Result<Json<CommonResponse>, (StatusCode, Json<GenericError>)> {
    // Validate request
    if request.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_NO_TOKENS_PROVIDED)),
                message: Some(Cow::Borrowed(MESSAGE_NO_TOKENS_PROVIDED)),
            }),
        ));
    }

    // 获取token manager的写锁
    let mut token_manager = state.token_manager_write().await;

    // 应用merge
    let mut updated_count = 0u32;
    let mut failed_count = 0u32;

    for (alias, token_info) in request {
        // 验证token是否在token_manager中存在
        if token_info.has_some()
            && let Some(mut writer) = token_manager
                .alias_map()
                .get(alias.as_str())
                .copied()
                .map(|id| unsafe { token_manager.tokens_mut().into_token_writer(id) })
        {
            let bundle = &mut **writer;
            if let Some(token) = token_info.primary_token {
                bundle.primary_token = token;
            }
            if let Some(token) = token_info.secondary_token {
                bundle.secondary_token = Some(token);
            }
            if let Some(checksum) = token_info.checksum {
                bundle.checksum = checksum;
            }
            if let Some(client_key) = token_info.client_key {
                bundle.client_key = client_key;
            }
            if let Some(config_version) = token_info.config_version {
                bundle.config_version = Some(config_version);
            }
            if let Some(session_id) = token_info.session_id {
                bundle.session_id = session_id;
            }
            if let Some(proxy) = token_info.proxy {
                bundle.proxy = Some(proxy);
            }
            if let Some(timezone) = token_info.timezone {
                bundle.timezone = Some(timezone);
            }
            if let Some(gcpp_host) = token_info.gcpp_host {
                bundle.gcpp_host = Some(gcpp_host);
            }
            updated_count += 1;
        } else {
            failed_count += 1;
        }
    }

    // 保存更改
    if updated_count > 0 && token_manager.save().await.is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_SAVE_TOKENS)),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_TOKEN_DATA_FAILED)),
            }),
        ));
    }

    Ok(Json(CommonResponse {
        status: ApiStatus::Success,
        message: Cow::Owned(
            [
                "已合并",
                itoa::Buffer::new().format(updated_count),
                "个令牌, ",
                itoa::Buffer::new().format(failed_count),
                "个令牌合并失败",
            ]
            .concat(),
        ),
    }))
}
