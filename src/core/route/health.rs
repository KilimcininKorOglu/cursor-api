use crate::{
    app::{
        frontend::metadata,
        lazy::START_TIME,
        model::{AppState, DateTime},
    },
    common::model::{
        ApiStatus,
        health::{
            Capabilities, CpuInfo, HealthCheckResponse, MemoryInfo, RequestStats, RuntimeStats,
            SystemStats, service_info,
        },
    },
    core::constant::Models,
};
use alloc::sync::Arc;
use axum::{Json, extract::State};
use core::sync::atomic::Ordering::Relaxed;
use manually_init::ManuallyInit;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Pid, RefreshKind, System};

type HashSet<K> = hashbrown::HashSet<K, ahash::RandomState>;

static ENDPOINTS: ManuallyInit<&'static [&'static str]> = ManuallyInit::new();

#[inline(always)]
pub fn init_endpoints(paths: HashSet<&'static str>) {
    let mut vec = Vec::from_iter(paths.iter().copied());
    vec.extend(crate::app::frontend::paths().filter(|p| !paths.contains(p)));
    ENDPOINTS.init(Box::leak(vec.into_boxed_slice()));
}

pub async fn handle_health(State(state): State<Arc<AppState>>) -> Json<HealthCheckResponse> {
    // Move system info collection to blocking thread pool
    let system = tokio::task::spawn_blocking(collect_system_stats).await.ok();

    Json(HealthCheckResponse {
        status: ApiStatus::Success,
        service: service_info(),
        frontend: metadata(),
        runtime: RuntimeStats {
            started_at: *START_TIME,
            uptime_seconds: (DateTime::naive_now() - START_TIME.naive()).num_seconds(),
            requests: RequestStats {
                total: state.total_requests.load(Relaxed),
                active: state.active_requests.load(Relaxed),
                errors: state.error_requests.load(Relaxed),
            },
        },
        system,
        capabilities: Capabilities {
            models: Models::get_ids_cache(),
            endpoints: &ENDPOINTS,
            features: &[
                #[cfg(feature = "horizon")]
                "horizon",
                #[cfg(feature = "__preview")]
                "preview",
                #[cfg(feature = "__compat")]
                "compat",
            ],
        },
    })
}

/// Collect system statistics (blocking operation)
fn collect_system_stats() -> SystemStats {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_memory(MemoryRefreshKind::everything())
            .with_cpu(CpuRefreshKind::everything()),
    );

    // CPU usage requires waiting for sampling interval
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);

    // Refresh system information
    sys.refresh_memory();
    sys.refresh_cpu_usage();

    let pid = std::process::id();
    let process = sys.process(Pid::from_u32(pid));

    // Get program memory usage and total system memory
    let memory_used = process.map(|p| p.memory()).unwrap_or(0);
    let total_memory = sys.total_memory();
    let available_memory = sys.available_memory();

    // Calculate memory usage percentage
    let memory_percentage =
        if total_memory > 0 { (memory_used as f32 / total_memory as f32) * 100.0 } else { 0.0 };

    // Get CPU usage
    let cpu_usage = sys.global_cpu_usage();

    // Get load average
    let load_avg = {
        let load = System::load_average();
        [load.one, load.five, load.fifteen]
    };

    SystemStats {
        memory: MemoryInfo {
            used_bytes: memory_used,
            used_percentage: memory_percentage,
            available_bytes: available_memory,
        },
        cpu: CpuInfo { usage_percentage: cpu_usage, load_average: load_avg },
    }
}
