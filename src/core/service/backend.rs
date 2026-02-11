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
    /// 初始状态，什么都未开始
    NotStarted,
    // /// message_start 已完成，等待 content_block_start
    // MessageStarted,
    /// content_block_start 已完成，正在接收 content_block_delta
    ContentBlockActive,
    // /// content_block_stop 已完成，等待下一个 content_block_start 或 message_delta
    // BetweenBlocks,
    // /// message_delta 已完成，等待 message_stop
    // MessageEnding,
    /// message_stop 已完成，Stream end
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
