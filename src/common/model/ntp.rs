use crate::app::constant::EMPTY_STRING;
use chrono::{DateTime, TimeDelta, Utc};
use core::{
    net::{Ipv4Addr, SocketAddrV4},
    sync::atomic::{AtomicI64, Ordering},
    time::Duration,
};
use manually_init::ManuallyInit;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;

const PORT: u16 = 123;
const TIMEOUT_SECS: u64 = 5;
const PACKET_SIZE: usize = 48;
const VERSION: u8 = 4;
const MODE_CLIENT: u8 = 3;
const MODE_SERVER: u8 = 4;
/// Seconds difference between January 1, 1900 and January 1, 1970
const EPOCH_DELTA: i64 = 0x83AA7E80;

static SERVERS: ManuallyInit<Servers> = ManuallyInit::new();
/// System time offset from accurate time (nanoseconds)
/// Satisfies: system time + DELTA = accurate time
pub static DELTA: AtomicI64 = AtomicI64::new(0);

// ========== Error type definitions ==========

#[derive(Debug)]
pub enum NtpError {
    /// NTP protocol layer error
    Protocol(&'static str),
    /// Network I/O error
    Io(std::io::Error),
    /// Request timeout
    Timeout,
    /// Time parse error
    TimeParse,
}

impl std::fmt::Display for NtpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NtpError::Protocol(msg) => write!(f, "NTP protocol error: {msg}"),
            NtpError::Io(e) => write!(f, "IO error: {e}"),
            NtpError::Timeout => write!(f, "NTP request timeout"),
            NtpError::TimeParse => write!(f, "Time parse error"),
        }
    }
}

impl std::error::Error for NtpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NtpError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for NtpError {
    fn from(e: std::io::Error) -> Self { NtpError::Io(e) }
}

impl From<tokio::time::error::Elapsed> for NtpError {
    fn from(_: tokio::time::error::Elapsed) -> Self { NtpError::Timeout }
}

// ========== Server list ==========

pub struct Servers {
    inner: Box<[String]>,
}

impl Servers {
    /// Initialize server list from environment variable NTP_SERVERS
    /// Format: comma-separated server addresses, e.g. "pool.ntp.org,time.cloudflare.com"
    pub fn init() {
        let env = crate::common::utils::parse_from_env("NTP_SERVERS", EMPTY_STRING);
        let servers: Vec<String> =
            env.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();

        SERVERS.init(Self { inner: servers.into_boxed_slice() });
    }
}

impl IntoIterator for &'static Servers {
    type Item = &'static str;
    type IntoIter =
        core::iter::Map<core::slice::Iter<'static, String>, fn(&'static String) -> &'static str>;

    fn into_iter(self) -> Self::IntoIter { self.inner.iter().map(String::as_str) }
}

// ========== Time conversion functions ==========

/// Convert NTP 64-bit timestamp to Unix DateTime
/// NTP timestamp format: high 32 bits are seconds, low 32 bits are fractional seconds
fn ntp_to_unix_timestamp(ntp_ts: u64) -> DateTime<Utc> {
    let ntp_secs = (ntp_ts >> 32) as i64;
    let ntp_frac = ntp_ts & 0xFFFFFFFF;
    let unix_secs = ntp_secs - EPOCH_DELTA;
    let nanos = ((ntp_frac * 1_000_000_000) >> 32) as u32;

    unsafe { DateTime::from_timestamp(unix_secs, nanos).unwrap_unchecked() }
}

/// Convert SystemTime to NTP 64-bit timestamp
fn system_time_to_ntp_timestamp(t: SystemTime) -> Result<u64, NtpError> {
    let duration = t.duration_since(UNIX_EPOCH).map_err(|_| NtpError::TimeParse)?;
    let secs = duration.as_secs() + EPOCH_DELTA as u64;
    let nanos = duration.subsec_nanos() as u64;
    let frac = (nanos << 32) / 1_000_000_000;

    Ok((secs << 32) | frac)
}

// ========== NTP timestamp structure ==========

/// Four key timestamps in NTP protocol
struct NtpTimestamps {
    t1: DateTime<Utc>, // Client send time
    t2: DateTime<Utc>, // Server receive time
    t3: DateTime<Utc>, // Server send time
    t4: DateTime<Utc>, // Client receive time
}

impl NtpTimestamps {
    /// Calculate clock offset
    /// Formula: θ = [(T2-T1) + (T4-T3)] / 2
    /// This formula assumes symmetric network delay (outbound and return are equal)
    fn clock_offset(&self) -> TimeDelta {
        let term1 = self.t2.signed_duration_since(self.t1);
        let term2 = self.t4.signed_duration_since(self.t3);
        (term1 + term2) / 2
    }

    /// Calculate round-trip delay (RTT)
    /// Formula: RTT = (T4-T1) - (T3-T2)
    #[allow(dead_code)]
    fn round_trip_delay(&self) -> TimeDelta {
        let total_time = self.t4.signed_duration_since(self.t1);
        let server_time = self.t3.signed_duration_since(self.t2);
        total_time - server_time
    }
}

/// Verify validity of NTP response packet
/// Check protocol version, mode, stratum level and other fields
#[inline]
fn validate_ntp_response(packet: [u8; 48]) -> Result<(), NtpError> {
    let mode = packet[0] & 0x7;
    let version = (packet[0] & 0x38) >> 3;
    let stratum = packet[1];

    let stratum_desc = match stratum {
        0 => "unspecified",
        1 => "primary reference source",
        2..=15 => "secondary server",
        16 => "unsynchronized",
        _ => "invalid",
    };

    crate::debug!("NTP response: version={version}, mode={mode}, stratum={stratum}({stratum_desc})");

    if mode != MODE_SERVER {
        return Err(NtpError::Protocol("Response mode is incorrect"));
    }

    match stratum {
        0 => Err(NtpError::Protocol("Server returned Kiss-o'-Death packet")),
        16 => Err(NtpError::Protocol("Server is unsynchronized")),
        17..=255 => Err(NtpError::Protocol("Server stratum value is invalid")),
        _ => Ok(()),
    }
}

// ========== Async UDP operations ==========

/// Create and bind UDP socket to random port
#[inline]
async fn create_udp_socket() -> Result<UdpSocket, NtpError> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).await?;
    Ok(socket)
}

/// Connect to available NTP server
/// Serially try each server in the server list until successful connection
async fn connect_to_ntp_server(socket: &UdpSocket) -> Result<&'static str, NtpError> {
    for server in SERVERS.get() {
        if socket.connect((server, PORT)).await.is_ok() {
            return Ok(server);
        }
    }
    Err(NtpError::Protocol("Unable to connect to any NTP server"))
}

/// Send NTP request and receive response
/// Returns: Response packet and four key timestamps
async fn send_and_receive_ntp_packet(
    socket: &UdpSocket,
) -> Result<([u8; PACKET_SIZE], NtpTimestamps), NtpError> {
    let mut packet = [0u8; PACKET_SIZE];
    packet[0] = (VERSION << 3) | MODE_CLIENT;

    // Record T1: client send time
    let t1_system = SystemTime::now();
    let t1_ntp = system_time_to_ntp_timestamp(t1_system)?;

    // Write T1 to packet's Transmit Timestamp Field (bytes 40-47)
    packet[40..48].copy_from_slice(&t1_ntp.to_be_bytes());

    // Send packet (with timeout protection)
    tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), socket.send(&packet)).await??;

    // Receive response (with timeout protection)
    tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), socket.recv(&mut packet)).await??;

    // Record T4: client receive time (as close to receive moment as possible)
    let t4_system = SystemTime::now();
    let t4_ntp = system_time_to_ntp_timestamp(t4_system)?;

    // Extract T2 and T3 from response packet
    let t2_ntp = u64::from_be_bytes(packet[32..40].try_into().unwrap());
    let t3_ntp = u64::from_be_bytes(packet[40..48].try_into().unwrap());

    Ok((
        packet,
        NtpTimestamps {
            t1: ntp_to_unix_timestamp(t1_ntp),
            t2: ntp_to_unix_timestamp(t2_ntp),
            t3: ntp_to_unix_timestamp(t3_ntp),
            t4: ntp_to_unix_timestamp(t4_ntp),
        },
    ))
}

// ========== Core synchronization functions ==========

/// Perform one NTP measurement
/// Returns: (clock offset nanoseconds, round-trip delay nanoseconds)
async fn measure_once() -> Result<(i64, i64), NtpError> {
    let socket = create_udp_socket().await?;
    let server = connect_to_ntp_server(&socket).await?;

    let (packet, timestamps) = send_and_receive_ntp_packet(&socket).await?;
    validate_ntp_response(packet)?;
    let offset = timestamps.clock_offset();
    let rtt = timestamps.round_trip_delay();
    let offset_nanos =
        offset.num_nanoseconds().ok_or(NtpError::Protocol("Time offset exceeds i64 range"))?;

    let rtt_nanos = rtt.num_nanoseconds().ok_or(NtpError::Protocol("RTT exceeds i64 range"))?;
    crate::debug!(
        "NTP sample: server={}, offset={}ms, RTT={}ms",
        server,
        offset_nanos / 1_000_000,
        rtt_nanos / 1_000_000
    );
    Ok((offset_nanos, rtt_nanos))
}

/// Perform one complete NTP synchronization process (multiple samples + weighted average)
///
/// Process:
/// 1. Call `measure_once()` multiple times based on configuration (default 8 times, 50ms interval)
/// 2. Collect successful samples
/// 3. Sort by RTT, filter out the largest ones
/// 4. Data cleaning: skip abnormal samples with RTT <= 0
/// 5. Weighted average of remaining samples by 1/RTT
///
/// Returns: Weighted average clock offset (nanoseconds)
pub async fn sync_once() -> Result<i64, NtpError> {
    let (sample_count, interval_ms) = parse_sample_config();
    let mut samples = Vec::with_capacity(sample_count);

    // 1. Multiple samples
    for i in 0..sample_count {
        match measure_once().await {
            Ok((delta, rtt)) => {
                samples.push((delta, rtt));
            }
            Err(e) => {
                crate::debug!("NTP sample failed ({}/{}): {}", i + 1, sample_count, e);
            }
        }

        // Sampling interval (no need to wait for the last one)
        if i + 1 < sample_count {
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        }
    }

    // 2. Check successful sample count
    let success_count = samples.len();
    if success_count < 3 {
        return Err(NtpError::Protocol("Fewer than 3 successful samples, cannot calculate reliable result"));
    }

    crate::debug!("NTP sampling completed: {success_count}/{sample_count} successful");

    // 3. Sort by RTT (ascending)
    samples.sort_by_key(|(_, rtt)| *rtt);

    // 4. Adaptive filtering: based on ratio strategy
    // Keep 70% of samples with smallest RTT, remove long-tail noise (network delay follows long-tail distribution)
    // Keep at least 4 samples to ensure statistical reliability, but not more than actual sample count
    const KEEP_RATIO: f64 = 0.70;
    const MIN_KEEP: usize = 4;

    let keep_count =
        ((success_count as f64 * KEEP_RATIO).ceil() as usize).max(MIN_KEEP).min(success_count);

    let valid_samples = &samples[..keep_count];
    let filter_count = success_count - keep_count;

    crate::debug!(
        "NTP filtering: samples={}, kept={}({:.0}%), filtered={}",
        success_count,
        keep_count,
        (keep_count as f64 / success_count as f64) * 100.0,
        filter_count
    );

    // 5. Data cleaning: skip abnormal samples with RTT <= 0 (already sorted, consecutive at front)
    let first_valid_idx = valid_samples
        .iter()
        .position(|(_, rtt)| *rtt > 0)
        .ok_or(NtpError::Protocol("All samples have abnormal RTT (<=0)"))?;

    if first_valid_idx > 0 {
        crate::debug!("Skipping {} samples with RTT<=0", first_valid_idx);
    }

    let clean_samples = &valid_samples[first_valid_idx..];

    // 6. Weighted average: weight = 1 / RTT
    let mut weighted_sum = 0.0f64;
    let mut weight_sum = 0.0f64;

    for (delta, rtt) in clean_samples {
        let weight = 1.0 / (*rtt as f64);
        weighted_sum += *delta as f64 * weight;
        weight_sum += weight;
    }

    let final_delta = (weighted_sum / weight_sum) as i64;

    // 7. Business reasonableness check: reject offsets exceeding ±1 day
    const MAX_REASONABLE_DELTA: i64 = 86400 * 1_000_000_000;

    if final_delta.abs() > MAX_REASONABLE_DELTA {
        crate::debug!("NTP offset exceeds reasonable range: δ={}s", final_delta / 1_000_000_000);
        return Err(NtpError::Protocol("Time offset exceeds reasonable range (±1 day)"));
    }

    // 8. Statistics information
    let min_rtt = clean_samples.first().map(|(_, rtt)| rtt / 1_000_000).unwrap_or(0);
    let max_rtt = clean_samples.last().map(|(_, rtt)| rtt / 1_000_000).unwrap_or(0);

    crate::debug!(
        "NTP sync completed: δ = {}ms, RTT range = [{}, {}]ms",
        final_delta / 1_000_000,
        min_rtt,
        max_rtt
    );

    Ok(final_delta)
}

// ========== Environment variable parsing ==========

/// Read sync interval from environment variable
/// Default value: 3600 seconds (1 hour)
fn parse_sync_interval() -> u64 {
    crate::common::utils::parse_from_env("NTP_SYNC_INTERVAL_SECS", 3600u64)
}

/// Parse sampling configuration
/// Returns: (sample count, sample interval milliseconds)
fn parse_sample_config() -> (usize, u64) {
    let count = crate::common::utils::parse_from_env("NTP_SAMPLE_COUNT", 8usize);
    let interval_ms = crate::common::utils::parse_from_env("NTP_SAMPLE_INTERVAL_MS", 50u64);
    (count, interval_ms)
}

// ========== Public interface ==========

/// Perform NTP synchronization once at startup
///
/// Behavior:
/// - No server configuration: silently return, DELTA remains 0
/// - Sync failed: print error to stdout, DELTA remains 0
/// - Sync successful: update DELTA, log, **automatically start periodic sync task**
pub async fn init_sync(stdout_ready: alloc::sync::Arc<tokio::sync::Notify>) {
    let servers = SERVERS.get();

    if servers.inner.is_empty() {
        stdout_ready.notified().await;
        __println!("\r\x1B[2KNo NTP server configured, inaccurate system time may cause undefined behavior");
        return;
    }

    let instant = std::time::Instant::now();

    match sync_once().await {
        Ok(delta_nanos) => {
            DELTA.store(delta_nanos, Ordering::Relaxed);
            let d = crate::common::utils::format_time_ms(instant.elapsed().as_secs_f64());
            stdout_ready.notified().await;
            println!("\r\x1B[2KNTP initialization completed: δ = {}ms, elapsed: {}s", delta_nanos / 1_000_000, d);

            // Automatically start periodic sync task after successful initialization
            spawn_periodic_sync();
        }
        Err(e) => {
            stdout_ready.notified().await;
            println!("\r\x1B[2KNTP sync failed: {e}");
        }
    }
}

/// Start periodic background sync task (automatically called by `init_sync`)
///
/// Startup conditions (must satisfy all):
/// 1. NTP servers configured (already checked by init_sync)
/// 2. Sync interval greater than 0 (NTP_SYNC_INTERVAL_SECS > 0)
///
/// Task behavior:
/// - Runs continuously in background
/// - Executes sync at configured interval
/// - First tick is skipped (startup sync already completed by init_sync)
/// - Sync failures logged to file, do not affect subsequent syncs
fn spawn_periodic_sync() {
    let interval_secs = parse_sync_interval();

    if interval_secs == 0 {
        return;
    }

    crate::debug!("Starting NTP periodic sync task: interval={interval_secs}s");

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

        // Skip first tick (startup sync already completed by init_sync)
        interval.tick().await;

        loop {
            interval.tick().await;

            match sync_once().await {
                Ok(delta_nanos) => {
                    DELTA.store(delta_nanos, Ordering::Relaxed);
                    crate::debug!("NTP periodic sync successful: δ = {}ms", delta_nanos / 1_000_000);
                }
                Err(e) => {
                    crate::debug!("NTP periodic sync failed: {e}");
                }
            }
        }
    });
}
