use crate::{
    app::model::{
        CommonResponse, ProxiesDeleteRequest, ProxiesDeleteResponse, ProxyAddRequest,
        ProxyInfoResponse, ProxyUpdateRequest, SetGeneralProxyRequest,
        proxy_pool::{self, Proxies},
    },
    common::model::{ApiStatus, GenericError},
};
use alloc::{borrow::Cow, sync::Arc};
use axum::{Json, http::StatusCode};
use interned::Str;

type HashMap<K, V> = hashbrown::HashMap<K, V, ahash::RandomState>;

crate::define_typed_constants! {
    &'static str => {
        // ERROR_SAVE_PROXY_CONFIG = "Failed to save proxy configuration: ",
        MESSAGE_SAVE_PROXY_CONFIG_FAILED = "Failed to save proxy configuration",
        ERROR_PROXY_NAME_NOT_FOUND = "Proxy name not found",
        MESSAGE_PROXY_NAME_NOT_FOUND = "Proxy name not found",
        MESSAGE_GENERAL_PROXY_SET = "General proxy has been set",
        MESSAGE_PROXY_CONFIG_UPDATED = "Proxy configuration updated",
        MESSAGE_NO_NEW_PROXY_ADDED = "No new proxies added",
        MESSAGE_ADDED_PREFIX = "Added ",
        MESSAGE_ADDED_SUFFIX = " new proxies",
    }
}

fn format_save_proxy_config_error(
    e: Box<dyn core::error::Error + Send + Sync + 'static>,
) -> String {
    format!("Failed to save proxy configuration: {e}")
}

// Get all proxy configurations
pub async fn handle_get_proxies() -> Json<ProxyInfoResponse> {
    // Get proxy configuration and release lock immediately
    let proxies = proxy_pool::proxies().load_full();

    let proxies_count = proxies.len();
    let general_proxy = proxy_pool::general_name().load_full();

    Json(ProxyInfoResponse {
        status: ApiStatus::Success,
        proxies: Some(proxies),
        proxies_count,
        general_proxy: Some(general_proxy),
        message: None,
    })
}

// Update proxy configuration
pub async fn handle_set_proxies(
    Json(proxies): Json<ProxyUpdateRequest>,
) -> Result<Json<ProxyInfoResponse>, (StatusCode, Json<GenericError>)> {
    // Update global proxy pool and save configuration
    proxies.update_global();
    if let Err(e) = Proxies::update_and_save().await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Owned(format_save_proxy_config_error(e))),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_PROXY_CONFIG_FAILED)),
            }),
        ));
    }

    // Get general proxy information (before updating app state)
    let proxies_count = proxy_pool::proxies().load().len();

    Ok(Json(ProxyInfoResponse {
        status: ApiStatus::Success,
        proxies: None,
        proxies_count,
        general_proxy: None,
        message: Some(Cow::Borrowed(MESSAGE_PROXY_CONFIG_UPDATED)),
    }))
}

// Add new proxies
pub async fn handle_add_proxy(
    Json(request): Json<ProxyAddRequest>,
) -> Result<Json<ProxyInfoResponse>, (StatusCode, Json<GenericError>)> {
    // Get current proxy configuration
    let current = proxy_pool::proxies().load_full();
    let proxies = request
        .proxies
        .into_iter()
        .filter(|(name, _)| !current.contains_key(name.as_str()))
        .collect::<HashMap<_, _>>();

    if proxies.is_empty() {
        // If no new proxies, return current state
        let proxies_count = current.len();

        return Ok(Json(ProxyInfoResponse {
            status: ApiStatus::Success,
            proxies: Some(current),
            proxies_count,
            general_proxy: None,
            message: Some(Cow::Borrowed(MESSAGE_NO_NEW_PROXY_ADDED)),
        }));
    }

    let mut current = (*current).clone();

    // Process new proxies
    let mut added_count = 0;

    for (name, proxy) in proxies {
        // Add new proxy directly
        current.insert(name.into(), proxy);
        added_count += 1;
    }

    // Update global proxy pool and save configuration
    proxy_pool::proxies().store(Arc::new(current));
    if let Err(e) = Proxies::update_and_save().await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Owned(format_save_proxy_config_error(e))),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_PROXY_CONFIG_FAILED)),
            }),
        ));
    }

    // Get updated information
    let proxies_count = proxy_pool::proxies().load().len();

    Ok(Json(ProxyInfoResponse {
        status: ApiStatus::Success,
        proxies: None,
        proxies_count,
        general_proxy: None,
        message: Some(Cow::Owned(
            [MESSAGE_ADDED_PREFIX, itoa::Buffer::new().format(added_count), MESSAGE_ADDED_SUFFIX]
                .concat(),
        )),
    }))
}

// Delete specified proxies
pub async fn handle_delete_proxies(
    Json(request): Json<ProxiesDeleteRequest>,
) -> Result<Json<ProxiesDeleteResponse>, (StatusCode, Json<GenericError>)> {
    let names = request.names;

    // Get current proxy configuration and calculate failed proxy names
    let current = proxy_pool::proxies().load_full();

    // Calculate failed proxy names
    let capacity = (names.len() * 3) >> 2;
    let mut processing_names: Vec<String> = Vec::with_capacity(capacity);
    let mut failed_names: Vec<String> = Vec::with_capacity(capacity);
    for name in names {
        if current.contains_key(name.as_str()) {
            processing_names.push(name);
        } else {
            failed_names.push(name);
        }
    }

    // Delete specified proxies
    if !processing_names.is_empty() {
        let mut map = current
            .iter()
            .filter_map(|(name, value)| {
                if !processing_names.iter().any(|s| *s == *name) {
                    Some((name.clone(), value.clone()))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>();
        if map.is_empty() {
            map = crate::app::model::proxy_pool::default_proxies();
        }
        proxy_pool::proxies().store(Arc::new(map));
    }

    // Update global proxy pool and save configuration
    if let Err(e) = Proxies::update_and_save().await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Owned(format_save_proxy_config_error(e))),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_PROXY_CONFIG_FAILED)),
            }),
        ));
    }

    // Return different results based on expectation
    let updated_proxies = if request.expectation.needs_updated_tokens() {
        Some(proxy_pool::proxies().load_full())
    } else {
        None
    };

    Ok(Json(ProxiesDeleteResponse {
        status: ApiStatus::Success,
        updated_proxies,
        failed_names: if request.expectation.needs_failed_tokens() && !failed_names.is_empty() {
            Some(failed_names)
        } else {
            None
        },
    }))
}

// Set general proxy
pub async fn handle_set_general_proxy(
    Json(request): Json<SetGeneralProxyRequest>,
) -> Result<Json<CommonResponse>, (StatusCode, Json<GenericError>)> {
    // Check if proxy name exists
    if !proxy_pool::proxies().load().contains_key(request.name.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Borrowed(ERROR_PROXY_NAME_NOT_FOUND)),
                message: Some(Cow::Borrowed(MESSAGE_PROXY_NAME_NOT_FOUND)),
            }),
        ));
    }

    // Set general proxy
    proxy_pool::general_name().store(Arc::new(Str::from(request.name)));

    // Update global proxy pool and save configuration
    if let Err(e) = Proxies::update_and_save().await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(GenericError {
                status: ApiStatus::Error,
                code: None,
                error: Some(Cow::Owned(format_save_proxy_config_error(e))),
                message: Some(Cow::Borrowed(MESSAGE_SAVE_PROXY_CONFIG_FAILED)),
            }),
        ));
    }

    Ok(Json(CommonResponse {
        status: ApiStatus::Success,
        message: Cow::Borrowed(MESSAGE_GENERAL_PROXY_SET),
    }))
}
