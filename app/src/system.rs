//! PC stats for the SYSTEM and GT widgets.

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{Disks, Networks, System};

#[derive(Clone, Default)]
pub struct SysStats {
    pub cpu: f32,
    /// None when no GPU reading is available
    pub gpu: Option<f32>,
    pub ram_used: u64,
    pub ram_total: u64,
    pub net_rx: u64, // bytes per second
    pub net_tx: u64,
    pub uptime: u64,
    pub disk_pct: u8,
}

impl SysStats {
    pub fn ram_pct(&self) -> u8 {
        if self.ram_total == 0 {
            0
        } else {
            (self.ram_used * 100 / self.ram_total) as u8
        }
    }
}

pub struct SysMonitor {
    sys: System,
    nets: Networks,
    disks: Disks,
    gpu: Arc<Mutex<Option<f32>>>,
    last: std::time::Instant,
}

impl SysMonitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let gpu = Arc::new(Mutex::new(None));
        spawn_gpu_reader(gpu.clone());
        SysMonitor {
            sys,
            nets: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
            gpu,
            last: std::time::Instant::now(),
        }
    }

    /// Call about once a second; CPU and network figures are deltas since the last call.
    pub fn sample(&mut self) -> SysStats {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.nets.refresh(true);
        self.disks.refresh(true);

        let secs = self.last.elapsed().as_secs_f64().max(0.1);
        self.last = std::time::Instant::now();
        let (rx, tx) = self
            .nets
            .iter()
            .fold((0, 0), |(r, t), (_, n)| (r + n.received(), t + n.transmitted()));

        // System drive, or the first disk
        let disk = self
            .disks
            .iter()
            .find(|d| d.mount_point().to_string_lossy().starts_with("C:"))
            .or_else(|| self.disks.iter().next());
        let disk_pct = disk
            .filter(|d| d.total_space() > 0)
            .map(|d| (100 - d.available_space() * 100 / d.total_space()) as u8)
            .unwrap_or(0);

        SysStats {
            cpu: self.sys.global_cpu_usage(),
            gpu: *self.gpu.lock().unwrap(),
            ram_used: self.sys.used_memory(),
            ram_total: self.sys.total_memory(),
            net_rx: (rx as f64 / secs) as u64,
            net_tx: (tx as f64 / secs) as u64,
            uptime: System::uptime(),
            disk_pct,
        }
    }
}

// ---------------------------------------------------------------------------
// GPU load. sysinfo has none, so ask the driver: nvidia-smi on NVIDIA cards,
// otherwise Windows' GPU engine counters (the number Task Manager shows).
// Each reading is a short one-shot process, so nothing outlives the app.
// ---------------------------------------------------------------------------

fn quiet(program: &str) -> Command {
    let mut c = Command::new(program);
    c.stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

fn nvidia_smi() -> Option<f32> {
    let out = quiet("nvidia-smi")
        .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits", "-i", "0"])
        .output()
        .ok()?;
    parse_number(&String::from_utf8_lossy(&out.stdout))
}

fn parse_number(s: &str) -> Option<f32> {
    s.trim().lines().next()?.trim().parse::<f32>().ok().map(|v| v.clamp(0.0, 100.0))
}

/// Sum of the 3D engine counters over every process, from typeperf's CSV
fn engine_counters() -> Option<f32> {
    let out = quiet("typeperf")
        .args([r"\GPU Engine(*engtype_3D)\Utilization Percentage", "-sc", "1"])
        .output()
        .ok()?;
    parse_typeperf(&String::from_utf8_lossy(&out.stdout))
}

fn parse_typeperf(csv: &str) -> Option<f32> {
    // Header row, then one row: "timestamp","v1","v2",...
    let row = csv.lines().filter(|l| l.starts_with('"')).nth(1)?;
    let sum: f32 = row.split(',').skip(1).filter_map(|v| v.trim_matches('"').parse::<f32>().ok()).sum();
    Some(sum.clamp(0.0, 100.0))
}

fn spawn_gpu_reader(out: Arc<Mutex<Option<f32>>>) {
    std::thread::spawn(move || {
        // Stick with the first source that answers; look again now and then
        // in case a driver shows up later
        let sources: [fn() -> Option<f32>; 2] = [nvidia_smi, engine_counters];
        let mut source: Option<fn() -> Option<f32>> = None;
        loop {
            let reading = match source {
                Some(f) => f(),
                None => sources.iter().find_map(|f| f().map(|v| (v, *f))).map(|(v, f)| {
                    source = Some(f);
                    v
                }),
            };
            if reading.is_none() {
                source = None;
            }
            *out.lock().unwrap() = reading;
            std::thread::sleep(Duration::from_secs(if source.is_some() { 2 } else { 60 }));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_output() {
        assert_eq!(parse_number("8\r\n"), Some(8.0));
        assert_eq!(parse_number("[N/A]"), None);
        let csv = "\r\n\"(PDH-CSV 4.0)\",\"\\\\PC\\GPU Engine(a)\",\"\\\\PC\\GPU Engine(b)\"\r\n\
                   \"11/01/2026 09:41:25.754\",\"12.5\",\"30.000000\"\r\nExiting, please wait...";
        assert_eq!(parse_typeperf(csv), Some(42.5));
        assert_eq!(parse_typeperf("Error: no such counter"), None);
    }
}
