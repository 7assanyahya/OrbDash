//! PC stats for the SYSTEM widget.

use sysinfo::{Disks, Networks, System};

#[derive(Clone, Default)]
pub struct SysStats {
    pub cpu: f32,
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
    last: std::time::Instant,
}

impl SysMonitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        SysMonitor {
            sys,
            nets: Networks::new_with_refreshed_list(),
            disks: Disks::new_with_refreshed_list(),
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
            ram_used: self.sys.used_memory(),
            ram_total: self.sys.total_memory(),
            net_rx: (rx as f64 / secs) as u64,
            net_tx: (tx as f64 / secs) as u64,
            uptime: System::uptime(),
            disk_pct,
        }
    }
}
