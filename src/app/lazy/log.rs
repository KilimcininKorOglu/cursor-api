use crate::common::utils::parse_from_env;
use alloc::borrow::Cow;
use core::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use manually_init::ManuallyInit;
use tokio::{
    fs::File,
    io::AsyncWriteExt as _,
    sync::{
        Mutex, OnceCell,
        mpsc::{self, UnboundedSender},
        watch,
    },
    task::JoinHandle,
};

// --- Global configuration ---

/// Control debug mode switch, read from environment variable "DEBUG", default to true
pub static DEBUG: ManuallyInit<bool> = ManuallyInit::new();

/// Path to debug log file, read from environment variable "DEBUG_LOG_FILE", default to "debug.log"
static DEBUG_LOG_FILE: ManuallyInit<Cow<'static, str>> = ManuallyInit::new();

/// Global log file handle
static LOG_FILE: ManuallyInit<Mutex<File>> = ManuallyInit::new();

/// Initialize log system configuration
///
/// Must be called once at program startup (before using logs)
#[forbid(unused)]
pub fn init() {
    DEBUG.init(parse_from_env("DEBUG", true));
    crate::common::model::health::init_service_info();

    // If debug not enabled, do not initialize log file
    if !*DEBUG {
        return;
    }

    DEBUG_LOG_FILE.init(parse_from_env("DEBUG_LOG_FILE", "debug.log"));

    // Synchronously open log file
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&**DEBUG_LOG_FILE)
        .expect("Fatal error: log system initialization failed - unable to open log file");

    // ConvertTo tokio 文件句柄
    LOG_FILE.init(Mutex::new(File::from_std(file)));
}

// --- 日志Message结构 ---

/// 带序列号的日志Message，EnsureHave序Handle
pub struct LogMessage {
    /// 全局递增的序列号，保证日志顺序
    pub seq: u64,
    /// AlreadyFormat化的日志Content（包含时间戳）
    pub content: String,
}

/// 全局日志序列号生成器（内部Use）
static LOG_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Get下一个日志序列号
#[inline]
fn next_log_seq() -> u64 { LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed) }

// --- 核心组件 ---

/// 全局单例的日志系统状态，Use OnceCell Ensure只初始化一次
static LOGGER_STATE: OnceCell<LoggerState> = OnceCell::const_new();

/// Log系统的状态结构，包含Send通道、关闭信号and后台任务句柄
pub struct LoggerState {
    /// 用于Send日志Message的无界通道Send端
    pub sender: UnboundedSender<LogMessage>,
    /// 用于Send关闭信号的 watch 通道Send端
    shutdown_tx: watch::Sender<bool>,
    /// 后台写入任务的句柄
    writer_handle: Mutex<Option<JoinHandle<()>>>,
}

/// Ensure日志系统Already初始化并返回其状态
///
/// If日志系统尚未初始化，会创建所需的通道and后台任务
///
/// Return日志系统状态的引用
pub fn ensure_logger_initialized() -> impl Future<Output = &'static LoggerState> {
    LOGGER_STATE.get_or_init(|| async {
        // Create用于传递日志Message的无界通道
        let (sender, mut receiver) = mpsc::unbounded_channel::<LogMessage>();
        // Create用于Send关闭信号的 watch 通道
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        // 启动后台写入任务
        let writer_handle = tokio::spawn(async move {
            // Configuration常Amount
            const BUFFER_CAPACITY: usize = 65536; // 64KB
            const MAX_PENDING_MESSAGES: usize = 1000;
            const OUT_OF_ORDER_THRESHOLD: u64 = 100;

            let mut buffer = Vec::<u8>::with_capacity(BUFFER_CAPACITY);
            // 定时刷新间隔
            let flush_interval = Duration::from_secs(3);
            let mut interval = tokio::time::interval(flush_interval);
            interval.tick().await; // 消耗初始 tick

            // 用于Cache乱序到达的Message
            let mut pending_messages = alloc::collections::BTreeMap::new();
            let mut next_seq = 0u64;

            // Main loop: handle log messages, periodic flush and shutdown signal
            loop {
                tokio::select! {
                    biased; // Prioritize handling branches above

                    // Receive new log message
                    Some(message) = receiver.recv() => {
                        // Add message to pending queue
                        pending_messages.insert(message.seq, message.content);

                        // Check if pending queue is too large
                        if pending_messages.len() > MAX_PENDING_MESSAGES {
                            let oldest_seq = *__unwrap!(pending_messages.keys().next());
                            eprintln!(
                                "Log system warning: too many pending messages (>{MAX_PENDING_MESSAGES}), force skip sequence {next_seq}-{}",
                                oldest_seq - 1
                            );
                            next_seq = oldest_seq;
                        }

                        // Handle all consecutive messages
                        while let Some(content) = pending_messages.remove(&next_seq) {
                            buffer.extend_from_slice(content.as_bytes());
                            buffer.push(b'\n');
                            next_seq += 1;

                            // Flush when buffer reaches capacity
                            if buffer.len() >= BUFFER_CAPACITY {
                                flush_byte_buffer(&mut buffer).await;
                                interval.reset();
                            }
                        }
                    }

                    // Periodic flush triggered
                    _ = interval.tick() => {
                        // During periodic flush, if there are pending messages and wait time is too long, force write
                        if !pending_messages.is_empty() {
                            let oldest_seq = *__unwrap!(pending_messages.keys().next());
                            // If oldest message sequence differs too much from expected, may have lost messages
                            if oldest_seq > next_seq + OUT_OF_ORDER_THRESHOLD {
                                eprintln!(
                                    "Log system warning: detected possible message loss, skip sequence {next_seq} to {}",
                                    oldest_seq - 1
                                );
                                next_seq = oldest_seq;
                            }
                        }
                        flush_byte_buffer(&mut buffer).await;
                    }

                    // 监听关闭信号
                    result = shutdown_rx.changed() => {
                        if result.is_err() || *shutdown_rx.borrow() {
                            // 接收剩余所HaveMessage
                            while let Ok(message) = receiver.try_recv() {
                                pending_messages.insert(message.seq, message.content);
                            }

                            // Handle所Have待Handle的Message并记录缺失范围
                            let mut missing_ranges = Vec::new();
                            for (seq, content) in pending_messages {
                                if seq != next_seq {
                                    missing_ranges.push((next_seq, seq - 1));
                                }
                                buffer.extend_from_slice(content.as_bytes());
                                buffer.push(b'\n');
                                next_seq = seq + 1;
                            }

                            // 报告缺失的日志
                            if !missing_ranges.is_empty() {
                                eprintln!("日志系统Warning：关闭时发现缺失的日志序号：");
                                for (start, end) in missing_ranges {
                                    if start == end {
                                        eprintln!("  序号 {start}");
                                    } else {
                                        eprintln!("  序号 {start}-{end}");
                                    }
                                }
                            }

                            // 最终刷新
                            flush_byte_buffer(&mut buffer).await;
                            break;
                        }
                    }

                    // 所Have其他Case（如通道关闭）
                    else => {
                        // Handle剩余的待HandleMessage
                        for (_, content) in pending_messages {
                            buffer.extend_from_slice(content.as_bytes());
                            buffer.push(b'\n');
                        }
                        flush_byte_buffer(&mut buffer).await;
                        break;
                    }
                }
            }
        });

        LoggerState {
            sender,
            shutdown_tx,
            writer_handle: Mutex::new(Some(writer_handle)),
        }
    })
}

/// 将缓冲区Content刷新到日志文件
///
/// # 参数
/// * `buffer` - 要写入的字节缓冲区，函数调用后会清Empty此缓冲区
async fn flush_byte_buffer(buffer: &mut Vec<u8>) {
    if buffer.is_empty() {
        return;
    }

    // Get日志文件的互斥锁
    let mut file_guard = LOG_FILE.lock().await;

    // Write数据
    if let Err(err) = file_guard.write_all(buffer).await {
        eprintln!("日志系统Error：写入日志数据Failed。Error：{err}");
        buffer.clear();
        return;
    }
    buffer.clear();

    // Ensure数据刷新到磁盘
    if let Err(err) = file_guard.flush().await {
        eprintln!("日志系统Error：刷新日志文件缓冲区到磁盘Failed。Error：{err}");
    }
}

// --- 公开接口 ---

/// 提交Debug日志到异步Handle队列
///
/// # 参数
/// * `seq` - 日志序列号
/// * `content` - AlreadyFormat化的日志Content
#[inline]
fn submit_debug_log(seq: u64, content: String) {
    tokio::spawn(async move {
        let state = ensure_logger_initialized().await;
        let log_msg = LogMessage { seq, content };
        if let Err(e) = state.sender.send(log_msg) {
            eprintln!("日志系统Error：Send日志Message至后台任务Failed。Error：{e}");
        }
    });
}

/// 记录Debug日志的宏
///
/// 仅当 DEBUG 开启时记录日志，异步Send到日志Handle任务
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if *$crate::app::lazy::log::DEBUG {
            $crate::app::lazy::log::debug_log(format_args!($($arg)*));
        }
    };
}

pub fn debug_log(args: core::fmt::Arguments<'_>) {
    // 立即Get序列号and时间戳，Ensure顺序性
    let seq = next_log_seq();
    let msg = format!(
        "{} | {}",
        crate::app::model::DateTime::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        args
    );
    submit_debug_log(seq, msg);
}

/// ProgramEnd前调用，Ensure所Have缓冲日志写入文件
///
/// Send关闭信号，等待后台写入任务Completed
pub async fn flush_all_debug_logs() {
    if let Some(state) = LOGGER_STATE.get() {
        if *DEBUG {
            __println!("日志系统：Start关闭流程...");
        }

        // Send关闭信号
        if let Err(err) = state.shutdown_tx.send(true)
            && *DEBUG
        {
            println!("日志系统Debug：Send关闭信号Failed（May写入任务Already提前End）：{err}");
        }

        // 提取后台任务句柄
        let handle = {
            let mut guard = state.writer_handle.lock().await;
            guard.take()
        };

        // 等待后台任务Completed，设置5秒超时
        if let Some(handle) = handle {
            match tokio::time::timeout(Duration::from_secs(5), handle).await {
                Ok(Ok(_)) => {
                    if *DEBUG {
                        __println!("日志系统：关闭Completed");
                    }
                }
                Ok(Err(join_err)) => {
                    eprintln!(
                        "日志系统Error：后台写入任务异常终止。部分日志May丢失。Error详情：{join_err}"
                    );
                }
                Err(_) => {
                    __eprintln!(
                        "日志系统Error：等待后台写入任务超时（5秒）。部分日志May未能写入。"
                    );
                }
            }
        } else if *DEBUG {
            __println!("日志系统Debug：未找到活动写入任务句柄，MayAlready关闭。");
        }
    } else if *DEBUG {
        __println!("日志系统Debug：日志系统未初始化，无需关闭。");
    }
}
