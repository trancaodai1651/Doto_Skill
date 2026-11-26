//! HardwareProbe: đọc thông tin phần cứng thật từ OS. Trường nào không đọc được → None + ghi chú,
//! không bao giờ bịa giá trị.

use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub vram_mb: Option<u64>,
    pub driver_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub kind: String,
    pub file_system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatteryInfo {
    pub present: bool,
    pub percent: Option<f32>,
    pub charging: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub hostname: String,
    pub cpu_brand: String,
    pub logical_cores: usize,
    pub physical_cores: Option<usize>,
    pub total_memory_mb: u64,
    pub gpus: Vec<GpuInfo>,
    pub disks: Vec<DiskInfo>,
    pub battery: Option<BatteryInfo>,
    pub probed_at: String,
    pub notes: Vec<String>,
}

pub fn vendor_of(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    if n.contains("nvidia") || n.contains("geforce") || n.contains("quadro") || n.contains("rtx") {
        "nvidia"
    } else if n.contains("amd") || n.contains("radeon") {
        "amd"
    } else if n.contains("intel") || n.contains("iris") || n.contains("uhd graphics") || n.contains("arc") {
        "intel"
    } else if n.contains("apple") {
        "apple"
    } else {
        "unknown"
    }
}

/// Chạy lệnh với timeout; trả stdout nếu thành công.
pub fn run_capture(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.stdout(std::process::Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                use std::io::Read;
                child.stdout.take()?.read_to_string(&mut out).ok()?;
                return status.success().then_some(out);
            }
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(30)),
            Err(_) => return None,
        }
    }
}

pub struct HardwareProbe;

impl HardwareProbe {
    /// Probe đầy đủ (có thể mất vài giây vì gọi PowerShell/system_profiler) — chạy trong spawn_blocking.
    pub fn probe() -> HardwareInfo {
        use sysinfo::{Disks, System};
        let mut notes = vec![];
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let cpu_brand = sys.cpus().first().map(|c| c.brand().trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "không xác định".into());
        let physical = sys.physical_core_count();
        if physical.is_none() {
            notes.push("OS không công bố số nhân vật lý".into());
        }
        let disks = Disks::new_with_refreshed_list()
            .list()
            .iter()
            .map(|d| DiskInfo {
                name: d.name().to_string_lossy().into_owned(),
                mount_point: d.mount_point().to_string_lossy().into_owned(),
                total_bytes: d.total_space(),
                available_bytes: d.available_space(),
                kind: match d.kind() {
                    sysinfo::DiskKind::SSD => "ssd",
                    sysinfo::DiskKind::HDD => "hdd",
                    _ => "unknown",
                }
                .into(),
                file_system: d.file_system().to_string_lossy().into_owned(),
            })
            .collect();
        let gpus = probe_gpus(&mut notes);
        let battery = probe_battery();
        HardwareInfo {
            os_name: System::name().unwrap_or_else(|| std::env::consts::OS.into()),
            os_version: System::os_version().unwrap_or_default(),
            arch: std::env::consts::ARCH.into(),
            hostname: System::host_name().unwrap_or_default(),
            cpu_brand,
            logical_cores: sys.cpus().len().max(1),
            physical_cores: physical,
            total_memory_mb: sys.total_memory() / 1024 / 1024,
            gpus,
            disks,
            battery,
            probed_at: doto_core::now_rfc3339(),
            notes,
        }
    }
}

fn probe_gpus(notes: &mut Vec<String>) -> Vec<GpuInfo> {
    let mut gpus = vec![];
    #[cfg(windows)]
    {
        let ps = "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM,DriverVersion | ConvertTo-Json -Compress";
        if let Some(out) = run_capture("powershell", &["-NoProfile", "-NonInteractive", "-Command", ps], Duration::from_secs(8)) {
            let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap_or(serde_json::Value::Null);
            let arr = match v {
                serde_json::Value::Array(a) => a,
                serde_json::Value::Null => vec![],
                other => vec![other],
            };
            for g in arr {
                let name = g["Name"].as_str().unwrap_or("").trim().to_string();
                if name.is_empty() {
                    continue;
                }
                // AdapterRAM là uint32 → sai với VRAM > 4 GB; chỉ dùng khi < 4 GB, còn lại để nvidia-smi bổ sung.
                let ram = g["AdapterRAM"].as_u64().filter(|b| *b > 0 && *b < 4_294_967_295).map(|b| b / 1024 / 1024);
                gpus.push(GpuInfo { vendor: vendor_of(&name).into(), name, vram_mb: ram, driver_version: g["DriverVersion"].as_str().map(String::from) });
            }
        } else {
            notes.push("không đọc được Win32_VideoController".into());
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(out) = run_capture("system_profiler", &["SPDisplaysDataType", "-json"], Duration::from_secs(10)) {
            let v: serde_json::Value = serde_json::from_str(&out).unwrap_or_default();
            for g in v["SPDisplaysDataType"].as_array().cloned().unwrap_or_default() {
                let name = g["sppci_model"].as_str().or(g["_name"].as_str()).unwrap_or("").to_string();
                if name.is_empty() {
                    continue;
                }
                let vram = g["spdisplays_vram"].as_str().or(g["spdisplays_vram_shared"].as_str()).and_then(parse_mem_string);
                gpus.push(GpuInfo { vendor: vendor_of(&name).into(), name, vram_mb: vram, driver_version: None });
            }
            if gpus.iter().any(|g| g.vendor == "apple") {
                notes.push("Apple Silicon dùng bộ nhớ hợp nhất (unified memory) — VRAM không tách riêng".into());
            }
        } else {
            notes.push("không đọc được system_profiler SPDisplaysDataType".into());
        }
    }
    // nvidia-smi cho VRAM/driver chính xác nếu có
    if let Some(out) = run_capture("nvidia-smi", &["--query-gpu=name,memory.total,driver_version", "--format=csv,noheader,nounits"], Duration::from_secs(5)) {
        for line in out.lines() {
            let parts: Vec<_> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 3 {
                let vram = parts[1].parse::<u64>().ok();
                if let Some(g) = gpus.iter_mut().find(|g| g.vendor == "nvidia" && g.name.contains(parts[0])) {
                    g.vram_mb = vram.or(g.vram_mb);
                    g.driver_version = Some(parts[2].to_string());
                } else {
                    gpus.push(GpuInfo { name: parts[0].into(), vendor: "nvidia".into(), vram_mb: vram, driver_version: Some(parts[2].into()) });
                }
            }
        }
    }
    if gpus.is_empty() {
        notes.push("không phát hiện GPU qua API của OS".into());
    }
    gpus
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_mem_string(s: &str) -> Option<u64> {
    let mut it = s.split_whitespace();
    let n: u64 = it.next()?.parse().ok()?;
    match it.next()?.to_ascii_uppercase().as_str() {
        "GB" => Some(n * 1024),
        "MB" => Some(n),
        _ => None,
    }
}

fn probe_battery() -> Option<BatteryInfo> {
    #[cfg(windows)]
    {
        let ps = "Get-CimInstance Win32_Battery | Select-Object EstimatedChargeRemaining,BatteryStatus | ConvertTo-Json -Compress";
        let out = run_capture("powershell", &["-NoProfile", "-NonInteractive", "-Command", ps], Duration::from_secs(6))?;
        let t = out.trim();
        if t.is_empty() {
            return Some(BatteryInfo { present: false, percent: None, charging: None });
        }
        let v: serde_json::Value = serde_json::from_str(t).ok()?;
        let v = if let serde_json::Value::Array(a) = v { a.into_iter().next()? } else { v };
        // BatteryStatus 2 = AC (đang sạc / cắm điện)
        return Some(BatteryInfo {
            present: true,
            percent: v["EstimatedChargeRemaining"].as_f64().map(|p| p as f32),
            charging: v["BatteryStatus"].as_u64().map(|s| s == 2 || s >= 6),
        });
    }
    #[cfg(target_os = "macos")]
    {
        let out = run_capture("pmset", &["-g", "batt"], Duration::from_secs(4))?;
        if !out.contains("InternalBattery") {
            return Some(BatteryInfo { present: false, percent: None, charging: None });
        }
        let percent = out.split(|c: char| c == '\t' || c == ';').find_map(|p| p.trim().strip_suffix('%').and_then(|n| n.parse::<f32>().ok()));
        return Some(BatteryInfo { present: true, percent, charging: Some(out.contains("AC Power")) });
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_detection() {
        assert_eq!(vendor_of("NVIDIA GeForce RTX 4070"), "nvidia");
        assert_eq!(vendor_of("AMD Radeon RX 7800"), "amd");
        assert_eq!(vendor_of("Intel(R) UHD Graphics 770"), "intel");
        assert_eq!(vendor_of("Apple M3 Pro"), "apple");
        assert_eq!(vendor_of("Microsoft Basic Display Adapter"), "unknown");
    }

    #[test]
    fn parses_mac_mem_strings() {
        assert_eq!(parse_mem_string("8 GB"), Some(8192));
        assert_eq!(parse_mem_string("1536 MB"), Some(1536));
        assert_eq!(parse_mem_string("lots"), None);
    }

    #[test]
    fn real_probe_returns_sane_values() {
        let hw = HardwareProbe::probe();
        assert!(hw.logical_cores >= 1);
        assert!(hw.total_memory_mb > 256);
        assert!(!hw.arch.is_empty());
    }
}
