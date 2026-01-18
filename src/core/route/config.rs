use crate::{
    app::{
        constant::header::CONFIG_HASH,
        lazy::CONFIG_FILE_PATH,
        model::{AppConfig, Hash},
    },
    common::model::HeaderValue,
};
use axum::{body::Body, response::IntoResponse};
use byte_str::ByteStr;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

pub async fn handle_get_config() -> impl IntoResponse {
    let (hash, content) = AppConfig::content();
    (
        [(CONFIG_HASH, {
            let mut buf = vec![0u8; 64];
            hash.to_str(unsafe { &mut *buf.as_mut_ptr().cast() });
            unsafe { HeaderValue { inner: Bytes::from(buf), is_sensitive: false }.into() }
        })],
        content.into_bytes(),
    )
}

fn checkhash(headers: HeaderMap) -> Result<(), (StatusCode, Body)> {
    let client_old_hash = headers
        .get(CONFIG_HASH)
        .ok_or((StatusCode::PRECONDITION_REQUIRED, "Missing config hash header".into()))?
        .to_str()
        .ok()
        .and_then(|s| s.parse::<Hash>().ok())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid config hash format".into()))?;

    if client_old_hash != AppConfig::hash() {
        return Err((StatusCode::PRECONDITION_FAILED, "Config has been modified by others".into()));
    }

    Ok(())
}

pub async fn handle_set_config(
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, Body)> {
    checkhash(headers)?;

    let content =
        ByteStr::from_utf8(body).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string().into()))?;

    let new_config =
        toml::from_str(&content).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string().into()))?;

    tokio::fs::write(&*CONFIG_FILE_PATH, content.clone().into_bytes())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string().into()))?;

    let changed = AppConfig::update(new_config, content);

    if changed { Ok(StatusCode::NO_CONTENT) } else { Ok(StatusCode::OK) }
}

pub async fn handle_reload_config(headers: HeaderMap) -> Result<StatusCode, (StatusCode, Body)> {
    checkhash(headers)?;

    let content =
        ByteStr::from(tokio::fs::read_to_string(&*CONFIG_FILE_PATH).await.map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read config file: {e}").into())
        })?);

    let new_config = toml::from_str(&content).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Config file parse error: {e}").into())
    })?;

    let changed = AppConfig::update(new_config, content);

    if changed { Ok(StatusCode::NO_CONTENT) } else { Ok(StatusCode::OK) }
}
