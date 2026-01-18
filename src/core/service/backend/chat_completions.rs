use super::{LastContentType, StreamState};
use crate::{
    app::{
        constant::{
            CHATCMPL_PREFIX, ERR_RESPONSE_RECEIVED, ERR_STREAM_RESPONSE,
            header::{CHUNKED, EVENT_STREAM, JSON, KEEP_ALIVE, NO_CACHE_REVALIDATE},
        },
        lazy::{REAL_USAGE, chat_url},
        model::{
            AppState, Chain, ChainUsage, DateTime, ErrorInfo, ExtToken, LogStatus, LogTokenInfo,
            LogUpdate, RequestLog, TimingInfo, log_manager,
        },
    },
    common::{
        client::{AiServiceRequest, build_client_request},
        model::{error::ChatError, tri::Tri},
        utils::{TrimNewlines as _, get_token_profile, get_token_usage, new_uuid_v4},
    },
    core::{
        adapter::openai::*,
        aiserver::v1::EnvironmentInfo,
        auth::TokenBundleResult,
        config::KeyConfig,
        error::{ErrorExt, StreamError},
        model::{ExtModel, MessageId, Role, openai::*},
        service::{UsageCheck, context::Tendency},
        stream::{
            decoder::{StreamDecoder, StreamMessage, Thinking},
            droppable::DroppableStream,
        },
    },
};
use alloc::{borrow::Cow, sync::Arc};
use atomic_enum::Atomic;
use axum::{Json, body::Body, response::Response};
use byte_str::ByteStr;
use bytes::Bytes;
use core::{
    convert::Infallible,
    sync::atomic::{AtomicU32, Ordering},
};
use futures_util::StreamExt as _;
use http::{
    Extensions, StatusCode,
    header::{CACHE_CONTROL, CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, TRANSFER_ENCODING},
};
use interned::Str;
use std::time::Instant;
use tokio::sync::Mutex;

struct Start {
    params: Vec<ChatCompletionMessageParam>,
    tools: Vec<ChatCompletionTool>,
    environment_info: EnvironmentInfo,
    current_config: KeyConfig,
    request_time: DateTime,
    ext_token: ExtToken,
    model: ExtModel,
    state: Arc<AppState>,
    current_id: u64,
    use_pri: bool,
    is_stream: bool,
    stream_options: ChatCompletionStreamOptions,
}

struct Continue {
    content: ChatCompletionContentText,
    tool_call_id: ByteStr,
    name: ByteStr,
    arguments: String,
    current_id: u64,
}

struct ChatCompletions;

impl ProtocolHandler for ChatCompletions {
    type Request = ChatCompletionCreateParams;
    type Error = OpenAiError;
    type Tendency = Tendency<Start, Continue>;
    async fn normalize_request(
        state: Arc<AppState>,
        mut extensions: Extensions,
        request: ChatCompletionCreateParams,
    ) -> Result<Tendency<Start, Continue>, (StatusCode, Json<OpenAiError>)> {
        let (ext_token, use_pri) = __unwrap!(extensions.remove::<TokenBundleResult>())
            .map_err(ErrorExt::into_openai_tuple)?;

        // 验证模型是否支持并获取模型信息
        let model = if let Some(model) = ExtModel::from_str(&request.model) {
            model
        } else {
            return Err(ChatError::ModelNotSupported(request.model).into_openai_tuple());
        };
        let (params, tools, is_stream, stream_options) = request.strip();

        if params.is_empty() {
            return Err(ChatError::EmptyMessages(StatusCode::BAD_REQUEST).into_openai_tuple());
        }

        let current_config = __unwrap!(extensions.remove::<KeyConfig>());
        let environment_info = __unwrap!(extensions.remove::<EnvironmentInfo>());
        let request_time = __unwrap!(extensions.remove::<DateTime>());

        let current_id: u64;
        let mut usage_check = None;

        // 更新请求日志
        state.increment_total();
        state.increment_active();
        if log_manager::is_enabled() {
            let next_id = log_manager::get_next_log_id().await;
            current_id = next_id;

            log_manager::add_log(
                RequestLog {
                    id: next_id,
                    timestamp: request_time,
                    model: model.id,
                    token_info: LogTokenInfo {
                        key: ext_token.primary_token.key(),
                        usage: None,
                        user: None,
                        stripe: None,
                    },
                    chain: Chain { delays: None, usage: None, think: None },
                    timing: TimingInfo { total: 0.0 },
                    stream: is_stream,
                    status: LogStatus::Pending,
                    error: ErrorInfo::Empty,
                },
                ext_token.clone(),
            )
            .await;

            // 如果需要获取用户使用情况,创建后台任务获取profile
            if model
                .is_usage_check(current_config.usage_check_models.as_ref().map(UsageCheck::from_pb))
            {
                let unext = ext_token.store_unext();
                let state = state.clone();
                let log_id = next_id;
                let client = ext_token.get_client_lazy();

                usage_check = Some(async move {
                    let (usage, stripe, user, ..) =
                        get_token_profile(client(), unext.as_ref(), use_pri, false).await;

                    // 更新日志中的profile
                    log_manager::update_log(
                        log_id,
                        LogUpdate::TokenProfile(user.clone(), usage, stripe),
                    )
                    .await;

                    let mut alias_updater = None;

                    // 更新token manager中的profile
                    if let Some(id) = {
                        state
                            .token_manager_read()
                            .await
                            .id_map()
                            .get(&unext.primary_token.key())
                            .copied()
                    } {
                        let alias_is_unnamed = unsafe {
                            state
                                .token_manager_read()
                                .await
                                .id_to_alias()
                                .get_unchecked(id)
                                .as_ref()
                                .unwrap_unchecked()
                                .is_unnamed()
                        };
                        let mut token_manager = state.token_manager_write().await;
                        let token_info =
                            unsafe { token_manager.tokens_mut().get_unchecked_mut(id) };
                        if alias_is_unnamed
                            && let Some(ref user) = user
                            && let Some(alias) = user.alias()
                        {
                            alias_updater = Some((id, alias.clone()));
                        }
                        token_info.user = user;
                        token_info.usage = usage;
                        token_info.stripe = stripe;
                    };

                    if let Some((id, alias)) = alias_updater {
                        let _ = state.token_manager_write().await.set_alias(id, alias);
                    }
                });
            }
        } else {
            current_id = 0;
        }

        Ok(match try_continue(params) {
            Ok((content, tool_call_id, name, arguments)) => {
                Tendency::Continue(Continue { content, tool_call_id, name, arguments, current_id })
            }
            Err(params) => Tendency::Start(Start {
                params,
                tools,
                environment_info,
                current_config,
                request_time,
                ext_token,
                model,
                state,
                current_id,
                use_pri,
                is_stream,
                stream_options,
            }),
        })
    }
}

fn try_continue(
    mut params: Vec<ChatCompletionMessageParam>,
) -> Result<(ChatCompletionContentText, ByteStr, ByteStr, String), Vec<ChatCompletionMessageParam>>
{
    fn check(params: &[ChatCompletionMessageParam]) -> bool {
        if let [
            ..,
            ChatCompletionMessageParam::Assistant { tool_calls: Some(tool_calls), .. },
            ChatCompletionMessageParam::Tool { content, tool_call_id },
        ] = params
            && let ChatCompletionMessageToolCall::Function {
                id,
                function: chat_completion_message_tool_call::Function { arguments, name },
            } = &**tool_calls
            && id == tool_call_id
        {
            true
        } else {
            false
        }
    }
    if check(&params) {
        if let (
            ChatCompletionMessageParam::Assistant { tool_calls: Some(tool_calls), .. },
            ChatCompletionMessageParam::Tool { content, tool_call_id },
        ) = unsafe {
            let len = params.len().unchecked_sub(2);
            params.set_len(len);
            core::hint::assert_unchecked(len < params.capacity());
            let ptr = params.as_ptr();
            (core::ptr::read(ptr.add(len)), core::ptr::read(ptr.add(len + 1)))
        } && let ChatCompletionMessageToolCall::Function {
            function: chat_completion_message_tool_call::Function { arguments, name },
            ..
        } = *tool_calls
        {
            Ok((content, tool_call_id, name, arguments))
        } else {
            __unreachable!()
        }
    } else {
        Err(params)
    }
}
