//! ResourceManager: 3 profile Nhẹ / Cân bằng / Tối đa + tuỳ chỉnh thủ công.
//! Profile thay đổi giới hạn thật mà Scheduler dùng (số job đồng thời, số thread FFmpeg,
//! ngưỡng CPU/RAM để nhận job, chu kỳ polling) và áp dụng ngay không cần restart.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Eco,
    #[default]
    Balanced,
    Max,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ResourceOverrides {
    pub max_jobs: Option<usize>,
    pub max_cpu_threads: Option<usize>,
    pub max_ram_mb: Option<u64>,
    pub max_gpu_jobs: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct StoredResourceSettings {
    pub profile: Profile,
    pub overrides: ResourceOverrides,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLimits {
    pub max_jobs: usize,
    pub max_cpu_threads: usize,
    pub max_ram_mb: u64,
    pub max_gpu_jobs: usize,
    pub ffmpeg_threads: usize,
    pub metrics_interval_ms: u64,
    pub cpu_admit_threshold: f32,
    pub min_free_ram_mb: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct HostCapacity {
    pub logical_cores: usize,
    pub total_memory_mb: u64,
}

/// Tính giới hạn hiệu lực từ profile + phần cứng + override (override bị kẹp trong giới hạn phần cứng).
pub fn compute_limits(profile: Profile, cap: HostCapacity, ov: &ResourceOverrides) -> ResourceLimits {
    let cores = cap.logical_cores.max(1);
    let mem = cap.total_memory_mb.max(512);
    let mut l = match profile {
        Profile::Eco => ResourceLimits {
            max_jobs: 1,
            max_cpu_threads: (cores / 4).max(1),
            max_ram_mb: mem / 4,
            max_gpu_jobs: 1,
            ffmpeg_threads: (cores / 4).max(1),
            metrics_interval_ms: 5000,
            cpu_admit_threshold: 60.0,
            min_free_ram_mb: (mem * 15 / 100).max(1024),
        },
        Profile::Balanced => ResourceLimits {
            max_jobs: (cores / 4).clamp(1, 3),
            max_cpu_threads: (cores / 2).max(2).min(cores),
            max_ram_mb: mem / 2,
            max_gpu_jobs: 2,
            ffmpeg_threads: (cores / 2).max(2).min(cores),
            metrics_interval_ms: 2000,
            cpu_admit_threshold: 85.0,
            min_free_ram_mb: (mem / 10).max(768),
        },
        Profile::Max => ResourceLimits {
            max_jobs: (cores / 2).clamp(2, 8),
            // giữ lại 1 nhân làm headroom cho UI/OS
            max_cpu_threads: cores.saturating_sub(1).max(1),
            max_ram_mb: mem * 3 / 4,
            max_gpu_jobs: 3,
            ffmpeg_threads: cores.saturating_sub(1).max(1),
            metrics_interval_ms: 1000,
            cpu_admit_threshold: 97.0,
            min_free_ram_mb: 512,
        },
    };
    if let Some(v) = ov.max_jobs {
        l.max_jobs = v.clamp(1, 32);
    }
    if let Some(v) = ov.max_cpu_threads {
        l.max_cpu_threads = v.clamp(1, cores);
        l.ffmpeg_threads = l.max_cpu_threads;
    }
    if let Some(v) = ov.max_ram_mb {
        l.max_ram_mb = v.clamp(256, mem);
    }
    if let Some(v) = ov.max_gpu_jobs {
        l.max_gpu_jobs = v.clamp(0, 8);
    }
    // FFmpeg thread mỗi job = tổng thread chia đều cho số job đồng thời
    l.ffmpeg_threads = (l.max_cpu_threads / l.max_jobs).max(1);
    l
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSample {
    pub cpu_percent: f32,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
    pub mem_available_mb: u64,
}

struct Inner {
    settings: StoredResourceSettings,
    limits: ResourceLimits,
    sample: SystemSample,
}

/// Quản lý tài nguyên dùng chung (clone rẻ).
#[derive(Clone)]
pub struct ResourceManager {
    cap: HostCapacity,
    inner: Arc<RwLock<Inner>>,
}

impl ResourceManager {
    pub fn new(cap: HostCapacity, settings: StoredResourceSettings) -> Self {
        let limits = compute_limits(settings.profile, cap, &settings.overrides);
        Self { cap, inner: Arc::new(RwLock::new(Inner { settings, limits, sample: SystemSample::default() })) }
    }

    pub fn detect_capacity() -> HostCapacity {
        let mut sys = sysinfo::System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        HostCapacity { logical_cores: sys.cpus().len().max(1), total_memory_mb: sys.total_memory() / 1024 / 1024 }
    }

    pub fn capacity(&self) -> HostCapacity {
        self.cap
    }

    pub fn settings(&self) -> StoredResourceSettings {
        self.inner.read().unwrap().settings.clone()
    }

    pub fn limits(&self) -> ResourceLimits {
        self.inner.read().unwrap().limits.clone()
    }

    pub fn sample(&self) -> SystemSample {
        self.inner.read().unwrap().sample
    }

    /// Đổi profile/override; áp dụng ngay cho các quyết định nhận job tiếp theo.
    pub fn apply(&self, settings: StoredResourceSettings) -> ResourceLimits {
        let limits = compute_limits(settings.profile, self.cap, &settings.overrides);
        let mut g = self.inner.write().unwrap();
        g.settings = settings;
        g.limits = limits.clone();
        limits
    }

    /// Cập nhật mẫu đo (dùng trong test hoặc sampler).
    pub fn set_sample(&self, s: SystemSample) {
        self.inner.write().unwrap().sample = s;
    }

    /// Quyết định có nhận thêm job không. `Err(lý do)` nếu phải chờ (backpressure).
    /// Luôn cho phép khi chưa có job nào chạy để tránh đói (starvation).
    pub fn admit(&self, running_jobs: usize, running_gpu_jobs: usize, wants_gpu: bool) -> Result<(), String> {
        let g = self.inner.read().unwrap();
        let l = &g.limits;
        if running_jobs >= l.max_jobs {
            return Err(format!("đạt giới hạn {} job đồng thời của profile", l.max_jobs));
        }
        if wants_gpu && running_gpu_jobs >= l.max_gpu_jobs {
            return Err(format!("đạt giới hạn {} phiên encoder GPU", l.max_gpu_jobs));
        }
        if running_jobs == 0 {
            return Ok(());
        }
        let s = &g.sample;
        if s.mem_total_mb > 0 && s.mem_available_mb < l.min_free_ram_mb {
            return Err(format!("RAM khả dụng {} MB thấp hơn ngưỡng an toàn {} MB", s.mem_available_mb, l.min_free_ram_mb));
        }
        if g.settings.profile != Profile::Max && s.cpu_percent > l.cpu_admit_threshold {
            return Err(format!("CPU {:.0}% vượt ngưỡng {:.0}% của profile", s.cpu_percent, l.cpu_admit_threshold));
        }
        Ok(())
    }

    /// Chạy vòng lặp lấy mẫu CPU/RAM theo chu kỳ của profile (Eco polling thưa hơn).
    pub fn spawn_sampler(&self) -> tokio::task::JoinHandle<()> {
        let me = self.clone();
        tokio::spawn(async move {
            let mut sys = sysinfo::System::new();
            loop {
                let interval = me.limits().metrics_interval_ms;
                let s = tokio::task::spawn_blocking(move || {
                    sys.refresh_cpu_usage();
                    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(250)));
                    sys.refresh_cpu_usage();
                    sys.refresh_memory();
                    let sample = SystemSample {
                        cpu_percent: sys.global_cpu_usage(),
                        mem_used_mb: sys.used_memory() / 1024 / 1024,
                        mem_total_mb: sys.total_memory() / 1024 / 1024,
                        mem_available_mb: sys.available_memory() / 1024 / 1024,
                    };
                    (sys, sample)
                })
                .await;
                match s {
                    Ok((back, sample)) => {
                        sys = back;
                        me.set_sample(sample);
                    }
                    Err(_) => {
                        sys = sysinfo::System::new();
                    }
                }
                tokio::time::sleep(Duration::from_millis(interval)).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: HostCapacity = HostCapacity { logical_cores: 16, total_memory_mb: 32768 };

    #[test]
    fn profiles_scale_concurrency_and_threads() {
        let eco = compute_limits(Profile::Eco, CAP, &Default::default());
        let bal = compute_limits(Profile::Balanced, CAP, &Default::default());
        let max = compute_limits(Profile::Max, CAP, &Default::default());
        assert!(eco.max_jobs < bal.max_jobs || eco.max_jobs == 1);
        assert!(bal.max_jobs <= max.max_jobs);
        assert!(eco.max_cpu_threads < bal.max_cpu_threads && bal.max_cpu_threads < max.max_cpu_threads);
        assert!(eco.metrics_interval_ms > bal.metrics_interval_ms && bal.metrics_interval_ms > max.metrics_interval_ms);
        assert_eq!(max.max_cpu_threads, 15, "Max giữ 1 nhân headroom");
        assert_eq!(eco.max_jobs, 1);
        assert_eq!(bal.max_jobs, 3);
        assert_eq!(max.max_jobs, 8);
    }

    #[test]
    fn overrides_are_clamped_to_hardware() {
        let ov = ResourceOverrides { max_jobs: Some(100), max_cpu_threads: Some(999), max_ram_mb: Some(1), max_gpu_jobs: Some(2) };
        let l = compute_limits(Profile::Balanced, CAP, &ov);
        assert_eq!(l.max_jobs, 32);
        assert_eq!(l.max_cpu_threads, 16);
        assert_eq!(l.max_ram_mb, 256);
        assert_eq!(l.max_gpu_jobs, 2);
    }

    #[test]
    fn tiny_machine_still_gets_one_worker() {
        let cap = HostCapacity { logical_cores: 1, total_memory_mb: 1024 };
        for p in [Profile::Eco, Profile::Balanced, Profile::Max] {
            let l = compute_limits(p, cap, &Default::default());
            assert!(l.max_jobs >= 1 && l.ffmpeg_threads >= 1 && l.max_cpu_threads >= 1);
        }
    }

    #[test]
    fn admission_respects_profile_live_change() {
        let rm = ResourceManager::new(CAP, StoredResourceSettings { profile: Profile::Eco, ..Default::default() });
        rm.set_sample(SystemSample { cpu_percent: 10.0, mem_used_mb: 1000, mem_total_mb: 32768, mem_available_mb: 30000 });
        assert!(rm.admit(0, 0, false).is_ok());
        assert!(rm.admit(1, 0, false).is_err(), "Eco chỉ 1 job");
        rm.apply(StoredResourceSettings { profile: Profile::Max, ..Default::default() });
        assert!(rm.admit(1, 0, false).is_ok(), "đổi profile áp dụng ngay");
    }

    #[test]
    fn admission_backpressure_on_cpu_and_ram() {
        let rm = ResourceManager::new(CAP, StoredResourceSettings::default());
        rm.set_sample(SystemSample { cpu_percent: 95.0, mem_used_mb: 1000, mem_total_mb: 32768, mem_available_mb: 30000 });
        assert!(rm.admit(1, 0, false).unwrap_err().contains("CPU"));
        assert!(rm.admit(0, 0, false).is_ok(), "không đói khi chưa có job");
        rm.set_sample(SystemSample { cpu_percent: 10.0, mem_used_mb: 32000, mem_total_mb: 32768, mem_available_mb: 200 });
        assert!(rm.admit(1, 0, false).unwrap_err().contains("RAM"));
        rm.set_sample(SystemSample { cpu_percent: 10.0, mem_used_mb: 1000, mem_total_mb: 32768, mem_available_mb: 30000 });
        assert!(rm.admit(1, 2, true).unwrap_err().contains("GPU"));
    }
}
