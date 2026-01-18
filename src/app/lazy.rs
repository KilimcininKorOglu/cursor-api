pub mod log;
mod path;

use super::{
    constant::{
        CURSOR_API2_HOST, CURSOR_API4_HOST, CURSOR_GCPP_ASIA_HOST, CURSOR_GCPP_EU_HOST,
        CURSOR_GCPP_US_HOST, CURSOR_HOST, EMPTY_STRING, HTTPS_PREFIX,
    },
    model::{DateTime, GcppHost},
};
use crate::common::utils::parse_from_env;
use alloc::borrow::Cow;
use manually_init::ManuallyInit;
pub use path::{
    CONFIG_FILE_PATH, DATA_DIR, LOGS_FILE_PATH, PROXIES_FILE_PATH, TOKENS_FILE_PATH,
    init as init_paths,
};
use std::sync::LazyLock;
use url::Url;

pub static AUTH_TOKEN: ManuallyInit<Cow<'static, str>> = ManuallyInit::new();

pub static START_TIME: ManuallyInit<DateTime> = ManuallyInit::new();

#[inline]
pub fn init_start_time() { START_TIME.init(DateTime::now()) }

#[inline]
pub fn init() {
    PRI_REVERSE_PROXY_HOST.init(parse_from_env("PRI_REVERSE_PROXY_HOST", EMPTY_STRING));
    PUB_REVERSE_PROXY_HOST.init(parse_from_env("PUB_REVERSE_PROXY_HOST", EMPTY_STRING));
    KEY_PREFIX.init(parse_from_env("KEY_PREFIX", DEFAULT_KEY_PREFIX));
    unsafe {
        USE_PRI_REVERSE_PROXY = !PRI_REVERSE_PROXY_HOST.is_empty();
        USE_PUB_REVERSE_PROXY = !PUB_REVERSE_PROXY_HOST.is_empty();
    }
    TCP_KEEPALIVE
        .init(parse_from_env("TCP_KEEPALIVE", DEFAULT_TCP_KEEPALIVE).min(MAX_TCP_KEEPALIVE));
    SERVICE_TIMEOUT
        .init(parse_from_env("SERVICE_TIMEOUT", DEFAULT_SERVICE_TIMEOUT).min(MAX_SERVICE_TIMEOUT));
    REAL_USAGE.init(parse_from_env("REAL_USAGE", true));
}

pub static GENERAL_TIMEZONE: LazyLock<chrono_tz::Tz> = LazyLock::new(|| {
    use std::str::FromStr as _;
    let tz = parse_from_env("GENERAL_TIMEZONE", EMPTY_STRING);
    if tz.is_empty() {
        __eprintln!(
            "未配置时区，请在环境变量GENERAL_TIMEZONE中设置，格式如'Asia/Shanghai'\n将使用默认时区: Asia/Shanghai"
        );
        chrono_tz::Tz::Asia__Shanghai
    } else {
        match chrono_tz::Tz::from_str(&tz) {
            Ok(tz) => tz,
            Err(e) => {
                eprintln!("无法解析时区 '{tz}': {e}\n将使用默认时区: Asia/Shanghai");
                chrono_tz::Tz::Asia__Shanghai
            }
        }
    }
});

pub static GENERAL_GCPP_HOST: LazyLock<GcppHost> = LazyLock::new(|| {
    let gcpp_host = parse_from_env("GENERAL_GCPP_HOST", EMPTY_STRING);
    if gcpp_host.is_empty() {
        __eprintln!(
            "未配置默认代码补全区域，请在环境变量GENERAL_GCPP_HOST中设置，格式如'Asia'\n将使用默认区域: Asia"
        );
        GcppHost::Asia
    } else {
        match GcppHost::from_str(&gcpp_host) {
            Some(gcpp_host) => gcpp_host,
            None => {
                eprintln!("无法解析区域 '{gcpp_host}'\n将使用默认区域: Asia");
                GcppHost::Asia
            }
        }
    }
});

pub static PRI_REVERSE_PROXY_HOST: ManuallyInit<Cow<'static, str>> = ManuallyInit::new();
pub static PUB_REVERSE_PROXY_HOST: ManuallyInit<Cow<'static, str>> = ManuallyInit::new();

const DEFAULT_KEY_PREFIX: &str = "sk-";
pub static KEY_PREFIX: ManuallyInit<Cow<'static, str>> = ManuallyInit::new();

pub static mut USE_PRI_REVERSE_PROXY: bool = false;
pub static mut USE_PUB_REVERSE_PROXY: bool = false;

macro_rules! def_cursor_api_url {
    (
        init_fn: $init_fn:ident,
        $(
            $group_name:ident => {
                host: $api_host:ident,
                apis: [
                    $( $name:ident => $path:expr ),+ $(,)?
                ]
            }
        ),+ $(,)?
    ) => {
        // 为每个API生成静态变量和getter函数
        $(
            $(
                paste::paste! {
                    static [<PRI_ $name:upper>]: ManuallyInit<Url> = ManuallyInit::new();
                    static [<PUB_ $name:upper>]: ManuallyInit<Url> = ManuallyInit::new();

                    #[inline(always)]
                    #[doc = $path]
                    pub fn $name(use_pri: bool) -> &'static Url {
                        if use_pri {
                            [<PRI_ $name:upper>].get()
                        } else {
                            [<PUB_ $name:upper>].get()
                        }
                    }
                }
            )+
        )+

        // 生成统一的初始化函数
        pub fn $init_fn() {
            $(
                $(
                    paste::paste! {
                        // 初始化私有URL
                        {
                            let host = if unsafe { USE_PRI_REVERSE_PROXY } {
                                &PRI_REVERSE_PROXY_HOST
                            } else {
                                $api_host
                            };
                            let mut url = String::with_capacity(HTTPS_PREFIX.len() + host.len() + $path.len());
                            url.push_str(HTTPS_PREFIX);
                            url.push_str(host);
                            url.push_str($path);
                            [<PRI_ $name:upper>].init(unsafe { Url::parse(&url).unwrap_unchecked() });
                        }

                        // 初始化公共URL
                        {
                            let host = if unsafe { USE_PUB_REVERSE_PROXY } {
                                &PUB_REVERSE_PROXY_HOST
                            } else {
                                $api_host
                            };
                            let mut url = String::with_capacity(HTTPS_PREFIX.len() + host.len() + $path.len());
                            url.push_str(HTTPS_PREFIX);
                            url.push_str(host);
                            url.push_str($path);
                            [<PUB_ $name:upper>].init(unsafe { Url::parse(&url).unwrap_unchecked() });
                        }
                    }
                )+
            )+
        }
    };
}

// 一次性定义所有API
def_cursor_api_url! {
    init_fn: init_all_cursor_urls,

    // API2 HOST 相关API
    api2_group => {
        host: CURSOR_API2_HOST,
        apis: [
            chat_url => "/aiserver.v1.ChatService/StreamUnifiedChatWithTools",
            chat_models_url => "/aiserver.v1.AiService/AvailableModels",
            stripe_url => "/auth/full_stripe_profile",
            token_poll_url => "/auth/poll",
            token_refresh_url => "/oauth/token",
            server_config_url => "/aiserver.v1.ServerConfigService/GetServerConfig",
            dry_chat_url => "/aiserver.v1.ChatService/GetPromptDryRun",
        ]
    },

    // CURSOR HOST 相关API
    cursor_group => {
        host: CURSOR_HOST,
        apis: [
            usage_api_url => "/api/usage-summary",
            user_api_url => "/api/dashboard/get-me",
            token_upgrade_url => "/api/auth/loginDeepCallbackControl",
            // teams_url => "/api/dashboard/teams",
            // aggregated_usage_events_url => "/api/dashboard/get-aggregated-usage-events",
            filtered_usage_events_url => "/api/dashboard/get-filtered-usage-events",
            sessions_url => "/api/auth/sessions",
            is_on_new_pricing_url => "/api/dashboard/is-on-new-pricing",
            get_privacy_mode_url => "/api/dashboard/get-user-privacy-mode",
        ]
    },

    // API4 HOST 相关API
    api4_group => {
        host: CURSOR_API4_HOST,
        apis: [
            cpp_config_url => "/aiserver.v1.AiService/CppConfig",
        ]
    },

    // API2 HOST CPP相关API
    api2_cpp_group => {
        host: CURSOR_API2_HOST,
        apis: [
            cpp_models_url => "/aiserver.v1.CppService/AvailableModels",
        ]
    },

    // GCPP ASIA HOST 相关API
    gcpp_asia_group => {
        host: CURSOR_GCPP_ASIA_HOST,
        apis: [
            asia_upload_file_url => "/aiserver.v1.FileSyncService/FSUploadFile",
            asia_sync_file_url => "/aiserver.v1.FileSyncService/FSSyncFile",
            asia_stream_cpp_url => "/aiserver.v1.AiService/StreamCpp",
            // asia_next_cursor_prediction_url => "/aiserver.v1.AiService/StreamNextCursorPrediction",
        ]
    },

    // GCPP EU HOST 相关API
    gcpp_eu_group => {
        host: CURSOR_GCPP_EU_HOST,
        apis: [
            eu_upload_file_url => "/aiserver.v1.FileSyncService/FSUploadFile",
            eu_sync_file_url => "/aiserver.v1.FileSyncService/FSSyncFile",
            eu_stream_cpp_url => "/aiserver.v1.AiService/StreamCpp",
            // eu_next_cursor_prediction_url => "/aiserver.v1.AiService/StreamNextCursorPrediction",
        ]
    },

    // GCPP US HOST 相关API
    gcpp_us_group => {
        host: CURSOR_GCPP_US_HOST,
        apis: [
            us_upload_file_url => "/aiserver.v1.FileSyncService/FSUploadFile",
            us_sync_file_url => "/aiserver.v1.FileSyncService/FSSyncFile",
            us_stream_cpp_url => "/aiserver.v1.AiService/StreamCpp",
            // us_next_cursor_prediction_url => "/aiserver.v1.AiService/StreamNextCursorPrediction",
        ]
    }
}

// TCP 和超时相关常量
const DEFAULT_TCP_KEEPALIVE: u64 = 90;
const MAX_TCP_KEEPALIVE: u64 = 600;
pub static TCP_KEEPALIVE: ManuallyInit<u64> = ManuallyInit::new();

const DEFAULT_SERVICE_TIMEOUT: u64 = 30;
const MAX_SERVICE_TIMEOUT: u64 = 600;
pub static SERVICE_TIMEOUT: ManuallyInit<u64> = ManuallyInit::new();

pub static REAL_USAGE: ManuallyInit<bool> = ManuallyInit::new();

// pub static TOKEN_VALIDITY_RANGE: ManuallyInit<TokenValidityRange> = ManuallyInit::new(|| {
//     let short = if let Ok(Ok(validity)) = std::env::var("TOKEN_SHORT_VALIDITY")
//         .as_deref()
//         .map(ValidityRange::from_str)
//     {
//         validity
//     } else {
//         ValidityRange::new(5400, 5400)
//     };
//     let long = if let Ok(Ok(validity)) = std::env::var("TOKEN_LONG_VALIDITY")
//         .as_deref()
//         .map(ValidityRange::from_str)
//     {
//         validity
//     } else {
//         ValidityRange::new(5184000, 5184000)
//     };
//     TokenValidityRange::new(short, long)
// });
