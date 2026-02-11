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

    // Convert to tokio file handle
    LOG_FILE.init(Mutex::new(File::from_std(file)));
}

// --- Log message structure ---

/// Log message with sequence number, ensures ordered processing
pub struct LogMessage {
    /// Globally incrementing sequence number, guarantees log order
    pub seq: u64,
    /// Already formatted log content (includes timestamp)
    pub content: String,
}

/// Global log sequence number generator (internal use)
static LOG_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Get next log sequence number
#[inline]
fn next_log_seq() -> u64 { LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed) }

// --- Core components ---

/// Global singleton log system state, uses OnceCell to ensure only initialized once
static LOGGER_STATE: OnceCell<LoggerState> = OnceCell::const_new();

/// Log system state structure, contains send channel, shutdown signal and background task handle
pub struct LoggerState {
    /// Unbounded channel sender for sending log messages
    pub sender: UnboundedSender<LogMessage>,
    /// Watch channel sender for sending shutdown signal
    shutdown_tx: watch::Sender<bool>,
    /// Background writer task handle
    writer_handle: Mutex<Option<JoinHandle<()>>>,
}

/// Ensure log system is initialized and return its state
///
/// If log system not yet initialized, creates required channels and background task
///
/// Returns reference to log system state
pub fn ensure_logger_initialized() -> impl Future<Output = &'static LoggerState> {
    LOGGER_STATE.get_or_init(|| async {
        // Create unbounded channel for passing log messages
        let (sender, mut receiver) = mpsc::unbounded_channel::<LogMessage>();
        // Create watch channel for sending shutdown signal
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        // Start background writer task
        let writer_handle = tokio::spawn(async move {
            // Configuration constants
            const BUFFER_CAPACITY: usize = 65536; // 64KB
            const MAX_PENDING_MESSAGES: usize = 1000;
            const OUT_OF_ORDER_THRESHOLD: u64 = 100;

            let mut buffer = Vec::<u8>::with_capacity(BUFFER_CAPACITY);
            // Periodic flush interval
            let flush_interval = Duration::from_secs(3);
            let mut interval = tokio::time::interval(flush_interval);
            interval.tick().await; // Consume initial tick

            // Cache for out-of-order messages
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

                    // Listen for shutdown signal
                    result = shutdown_rx.changed() => {
                        if result.is_err() || *shutdown_rx.borrow() {
                            // Receive all remaining messages
                            while let Ok(message) = receiver.try_recv() {
                                pending_messages.insert(message.seq, message.content);
                            }

                            // Handle all pending messages and record missing ranges
                            let mut missing_ranges = Vec::new();
                            for (seq, content) in pending_messages {
                                if seq != next_seq {
                                    missing_ranges.push((next_seq, seq - 1));
                                }
                                buffer.extend_from_slice(content.as_bytes());
                                buffer.push(b'\n');
                                next_seq = seq + 1;
                            }

                            // Report missing logs
                            if !missing_ranges.is_empty() {
                                eprintln!("Log system warning: missing log sequence numbers found during shutdown:");
                                for (start, end) in missing_ranges {
                                    if start == end {
                                        eprintln!("  Sequence {start}");
                                    } else {
                                        eprintln!("  Sequence {start}-{end}");
                                    }
                                }
                            }

                            // Final flush
                            flush_byte_buffer(&mut buffer).await;
                            break;
                        }
                    }

                    // All other cases (e.g. channel closed)
                    else => {
                        // Handle remaining pending messages
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

/// Flush buffer content to log file
///
/// # Arguments
/// * `buffer` - Byte buffer to write, will be cleared after function call
async fn flush_byte_buffer(buffer: &mut Vec<u8>) {
    if buffer.is_empty() {
        return;
    }

    // Get mutex lock for log file
    let mut file_guard = LOG_FILE.lock().await;

    // Write data
    if let Err(err) = file_guard.write_all(buffer).await {
        eprintln!("Log system error: failed to write log data. Error: {err}");
        buffer.clear();
        return;
    }
    buffer.clear();

    // Ensure data is flushed to disk
    if let Err(err) = file_guard.flush().await {
        eprintln!("Log system error: failed to flush log file buffer to disk. Error: {err}");
    }
}

// --- Public interface ---

/// Submit debug log to async processing queue
///
/// # Arguments
/// * `seq` - Log sequence number
/// * `content` - Already formatted log content
#[inline]
fn submit_debug_log(seq: u64, content: String) {
    tokio::spawn(async move {
        let state = ensure_logger_initialized().await;
        let log_msg = LogMessage { seq, content };
        if let Err(e) = state.sender.send(log_msg) {
            eprintln!("Log system error: failed to send log message to background task. Error: {e}");
        }
    });
}

/// Macro for recording debug logs
///
/// Only records logs when DEBUG is enabled, asynchronously sends to log processing task
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if *$crate::app::lazy::log::DEBUG {
            $crate::app::lazy::log::debug_log(format_args!($($arg)*));
        }
    };
}

pub fn debug_log(args: core::fmt::Arguments<'_>) {
    // Immediately get sequence number and timestamp to ensure ordering
    let seq = next_log_seq();
    let msg = format!(
        "{} | {}",
        crate::app::model::DateTime::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        args
    );
    submit_debug_log(seq, msg);
}

/// Call before program ends to ensure all buffered logs are written to file
///
/// Sends shutdown signal, waits for background writer task to complete
pub async fn flush_all_debug_logs() {
    if let Some(state) = LOGGER_STATE.get() {
        if *DEBUG {
            __println!("Log system: starting shutdown process...");
        }

        // Send shutdown signal
        if let Err(err) = state.shutdown_tx.send(true)
            && *DEBUG
        {
            println!("Log system debug: failed to send shutdown signal (writer task may have ended early): {err}");
        }

        // Extract background task handle
        let handle = {
            let mut guard = state.writer_handle.lock().await;
            guard.take()
        };

        // Wait for background task to complete, with 5 second timeout
        if let Some(handle) = handle {
            match tokio::time::timeout(Duration::from_secs(5), handle).await {
                Ok(Ok(_)) => {
                    if *DEBUG {
                        __println!("Log system: shutdown complete");
                    }
                }
                Ok(Err(join_err)) => {
                    eprintln!(
                        "Log system error: background writer task terminated abnormally. Some logs may be lost. Error details: {join_err}"
                    );
                }
                Err(_) => {
                    __eprintln!(
                        "Log system error: timeout waiting for background writer task (5 seconds). Some logs may not have been written."
                    );
                }
            }
        } else if *DEBUG {
            __println!("Log system debug: no active writer task handle found, may have already shut down.");
        }
    } else if *DEBUG {
        __println!("Log system debug: log system not initialized, no need to shut down.");
    }
}
