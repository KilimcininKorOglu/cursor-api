mod chat_completions;
mod messages;
mod responses;

use super::context::Tendency;
use crate::app::model::AppState;
use alloc::sync::Arc;
use atomic_enum::atomic_enum;
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use http::{Extensions, StatusCode};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
enum StreamState {
    /// Initial state, nothing started yet
    NotStarted,
    // /// message_start completed, waiting for content_block_start
    // MessageStarted,
    /// content_block_start completed, receiving content_block_delta
    ContentBlockActive,
    // /// content_block_stop completed, waiting for next content_block_start or message_delta
    // BetweenBlocks,
    // /// message_delta completed, waiting for message_stop
    // MessageEnding,
    /// message_stop completed, stream end
    Completed,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq)]
enum LastContentType {
    None,
    Thinking,
    Text,
    InputJson,
}

atomic_enum!(StreamState = u8);
atomic_enum!(LastContentType = u8);

trait ProtocolHandler: Sized {
    type Request: serde::de::DeserializeOwned;
    type Error: serde::ser::Serialize;
    type Tendency;
    async fn normalize_request(
        state: Arc<AppState>,
        extensions: Extensions,
        request: Self::Request,
    ) -> Result<Self::Tendency, (StatusCode, Json<Self::Error>)>;
    async fn denormalize_response(
        tendency: Self::Tendency,
    ) -> Result<Response, (StatusCode, Json<Self::Error>)>;
    async fn check_session_status() -> bool;

    async fn handle(
        State(state): State<Arc<AppState>>,
        extensions: Extensions,
        Json(request): Json<Self::Request>,
    ) -> Result<Response, (StatusCode, Json<Self::Error>)> {
        let tendency = Self::normalize_request(state, extensions, request).await?;
        Self::denormalize_response(tendency).await
    }
}
