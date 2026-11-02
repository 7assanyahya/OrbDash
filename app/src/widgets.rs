//! The board widgets: settings, the serial lines that feed them, and a
//! preview that mirrors the firmware's layout (240x240 coordinates).

use crate::claude::ClaudeStats;
use crate::custom::{CmdOutput, CustomPage};
use crate::system::SysStats;
use chrono::{Datelike, Local, Timelike, Utc};
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

/// Board widgets in firmware order. GT came after Custom so older boards keep
/// their numbering.
pub const MODE_NAMES: [&str; 8] = [
    "Claude 5H",
    "Claude Today",
    "Claude Timer",
    "Claude All",
    "Clock",
    "System",
    "Custom",
    "GT Gauge",
];
pub const MODE_CLOCK: usize = 4;
pub const MODE_SYSTEM: usize = 5;
pub const MODE_CUSTOM: usize = 6;
pub const MODE_GT: usize = 7;
/// Built-in widgets in the order the app lists them (Custom has its pages)
pub const BUILTINS: [usize; 7] = [0, 1, 2, 3, 4, 5, MODE_GT];
/// GT gauge redline, as in the firmware
pub const GT_RED_FROM: u8 = 85;

// Firmware palette (RGB565 -> RGB)
pub const CLAUDE: Color32 = Color32::from_rgb(0xD9, 0x77, 0x57);
pub const DIM: Color32 = Color32::from_rgb(123, 125, 123);
pub const TRACK: Color32 = Color32::from_rgb(24, 28, 24);
pub const TEXT: Color32 = Color32::WHITE;
pub const CYAN: Color32 = Color32::from_rgb(0, 255, 255);
pub const LIVE: Color32 = Color32::from_rgb(0, 255, 0);
pub const WARN: Color32 = Color32::from_rgb(255, 165, 0);
pub const ALERT: Color32 = Color32::from_rgb(255, 0, 0);
pub const PURPLE: Color32 = Color32::from_rgb(156, 105, 255);

/// Which OrbDash you have. Learned from the board when it connects (its
/// ORB|READY line), and picked in Settings for the preview while offline.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Device {
    Classic,
    Mini,
    Pro,
}

impl Device {
    pub const ALL: [Device; 3] = [Device::Classic, Device::Mini, Device::Pro];

    pub fn name(self) -> &'static str {
        match self {
            Device::Classic => "OrbDash",
            Device::Mini => "OrbDash Mini",
            Device::Pro => "OrbDash Pro",
        }
    }

    pub fn board(self) -> &'static str {
        match self {
            Device::Classic => "Arduino Uno",
            Device::Mini => "ESP32-C3 SuperMini",
            Device::Pro => "ESP32-S3 DevKit",
        }
    }

    pub fn screens(self) -> usize {
        if self == Device::Pro { 3 } else { 1 }
    }

    /// From the device field of ORB|READY ("MINI", "PRO", or none for the Uno)
    pub fn from_code(code: Option<&str>) -> Device {
        match code.map(|c| c.trim().to_ascii_uppercase()).as_deref() {
            Some("PRO") => Device::Pro,
            Some("MINI") => Device::Mini,
            _ => Device::Classic,
        }
    }
}

/// Short name of a screen on a multi-screen device
pub fn screen_side(screens: usize, i: usize) -> String {
    match (screens, i) {
        (3, 0) => "Left".into(),
        (3, 1) => "Middle".into(),
        (3, 2) => "Right".into(),
        _ => format!("Screen {}", i + 1),
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum ClockExtra {
    Utc,
    Week,
    Text,
    Nothing,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub port: String,
    pub auto_connect: bool,
    pub app_rotate: bool,
    pub rotate_secs: u32,
    /// Per widget, in firmware order. A Vec so settings saved with fewer
    /// widgets still load.
    pub playlist: Vec<bool>,
    pub screen_rotation: u8,
    /// Token budget per 5h block in millions; 0 = ring shows time elapsed
    pub block_limit_m: f32,
    pub daily_goal_m: f32,
    pub clock_24h: bool,
    pub clock_extra: ClockExtra,
    pub clock_text: String,
    pub clock_color: [u8; 3],
    pub sys_color: [u8; 3],
    pub customs: Vec<CustomPage>,
    pub next_id: u64,
    pub start_with_windows: bool,
    pub device: Device,
    /// Widget on each screen of a multi-screen device, restored on connect
    pub layout: Vec<usize>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            port: String::new(),
            auto_connect: true,
            app_rotate: false,
            rotate_secs: 10,
            playlist: vec![true, false, false, false, true, true, false, false],
            screen_rotation: 0,
            block_limit_m: 0.0,
            daily_goal_m: 2.0,
            clock_24h: true,
            clock_extra: ClockExtra::Utc,
            clock_text: String::new(),
            clock_color: [0, 255, 255],
            sys_color: [0, 255, 0],
            customs: vec![CustomPage { id: 1, ..CustomPage::default() }],
            next_id: 2,
            start_with_windows: false,
            device: Device::Classic,
            // Pro: Claude 5H on the left, GT gauge in the middle, clock on the right
            layout: vec![0, MODE_GT, MODE_CLOCK],
        }
    }
}

/// Everything the widgets read from
pub struct Data<'a> {
    pub claude: &'a ClaudeStats,
    pub sys: &'a SysStats,
    /// Custom page on the board's CUSTOM slot, and its command output
    pub page: Option<&'a CustomPage>,
    pub cmd: Option<&'a CmdOutput>,
    pub s: &'a Settings,
}

pub fn human(n: u64) -> String {
    for (unit, size) in [("B", 1e9), ("M", 1e6), ("K", 1e3)] {
        let v = n as f64 / size;
        if v >= 1.0 {
            return if v < 100.0 { format!("{v:.1}{unit}") } else { format!("{v:.0}{unit}") };
        }
    }
    n.to_string()
}

/// Make a field safe for the board: ASCII only, no separators, never empty
pub fn field(s: &str, max: usize) -> String {
    let s: String = s
        .chars()
        .filter(|c| c.is_ascii() && !c.is_ascii_control() && *c != '|')
        .take(max)
        .collect();
    if s.is_empty() { " ".into() } else { s }
}

pub fn to565(c: [u8; 3]) -> u16 {
    ((c[0] as u16 >> 3) << 11) | ((c[1] as u16 >> 2) << 5) | (c[2] as u16 >> 3)
}

// ---------------------------------------------------------------------------
// Derived values
// ---------------------------------------------------------------------------

impl Data<'_> {
    pub fn block_pct(&self) -> u8 {
        let c = self.claude;
        if c.block_start.is_none() {
            0
        } else if self.s.block_limit_m > 0.0 {
            (c.block_tokens as f64 * 100.0 / (self.s.block_limit_m as f64 * 1e6)).min(100.0) as u8
        } else {
            c.block_elapsed_pct()
        }
    }

    pub fn today_pct(&self) -> u8 {
        if self.s.daily_goal_m <= 0.0 {
            return 0;
        }
        (self.claude.today_tokens as f64 * 100.0 / (self.s.daily_goal_m as f64 * 1e6)).min(100.0) as u8
    }

    pub fn timer_pct(&self) -> u8 {
        match self.claude.block_start {
            Some(_) => 100 - self.claude.block_elapsed_pct(),
            None => 0,
        }
    }

    fn clock(&self) -> (String, u32, String, String, String) {
        let now = Local::now();
        let time = if self.s.clock_24h {
            now.format("%H:%M").to_string()
        } else {
            now.format("%I:%M").to_string()
        };
        let date = now.format("%d %b").to_string().to_uppercase();
        let day = now.format("%A").to_string().to_uppercase();
        let extra = match self.s.clock_extra {
            ClockExtra::Utc => format!("UTC {}", Utc::now().format("%H:%M")),
            ClockExtra::Week => format!("WEEK {}  DAY {}", now.iso_week().week(), now.ordinal()),
            ClockExtra::Text => self.s.clock_text.clone(),
            ClockExtra::Nothing => String::new(),
        };
        let extra = if self.s.clock_24h || self.s.clock_extra != ClockExtra::Nothing {
            extra
        } else {
            now.format("%p").to_string()
        };
        (time, now.second(), date, day, extra)
    }

    fn sys_lines(&self) -> (String, String, String) {
        let s = self.sys;
        let ram = format!("{:.1}/{:.0} GB", s.ram_used as f64 / 1e9, s.ram_total as f64 / 1e9);
        let net = format!("NET D {}  U {}", human(s.net_rx), human(s.net_tx));
        let up = s.uptime;
        let line2 = format!("UP {}d{:02}h  DISK {}%", up / 86400, up / 3600 % 24, s.disk_pct);
        (ram, net, line2)
    }

    fn custom(&self) -> (String, String, String, String, u8, [u8; 3]) {
        match self.page {
            Some(p) => {
                let r = p.resolve(self.cmd);
                (r.title, r.value, r.subtitle, r.line, r.pct, r.color)
            }
            None => ("CUSTOM".into(), "--".into(), "ADD A PAGE".into(), "in the app".into(), 0, [156, 105, 255]),
        }
    }

    // -----------------------------------------------------------------------
    // Serial lines (field limits match the firmware's buffers)
    // -----------------------------------------------------------------------

    pub fn claude_line(&self) -> String {
        let c = self.claude;
        format!(
            "U|{}|{}|{}|{}|{}|{}",
            field(&human(c.block_tokens), 7),
            self.block_pct(),
            field(&human(c.today_tokens), 7),
            field(&c.today_msgs.to_string(), 7),
            field(&c.time_left(), 9),
            self.today_pct()
        )
    }

    pub fn clock_line(&self) -> String {
        let (time, sec, date, day, extra) = self.clock();
        format!(
            "C|{}|{:02}|{}|{}|{}|{:04X}",
            field(&time, 5),
            sec,
            field(&date, 7),
            field(&day, 9),
            field(&extra, 22),
            to565(self.s.clock_color)
        )
    }

    pub fn system_line(&self) -> String {
        let (ram, l1, l2) = self.sys_lines();
        format!(
            "S|{}|{}|{}|{}|{}|{:04X}",
            self.sys.cpu.round() as u32,
            self.sys.ram_pct(),
            field(&ram, 15),
            field(&l1, 22),
            field(&l2, 20),
            to565(self.s.sys_color)
        )
    }

    pub fn gt_line(&self) -> String {
        let gpu = self.sys.gpu.map(|g| (g.round() as u32).to_string()).unwrap_or("-".into());
        format!("G|{}|{gpu}|{}", self.sys.cpu.round() as u32, self.sys.ram_pct())
    }

    pub fn custom_line(&self) -> String {
        let (title, value, sub, line, pct, color) = self.custom();
        format!(
            "M|{}|{}|{}|{}|{}|{:04X}",
            field(&title, 11),
            field(&value, 8),
            field(&sub, 18),
            field(&line, 22),
            pct,
            to565(color)
        )
    }

    // -----------------------------------------------------------------------
    // Preview: same text positions as drawStaticUI/drawReadout in the firmware
    // -----------------------------------------------------------------------

    pub fn view(&self, mode: usize) -> View {
        let c = self.claude;
        let mut v = View::default();
        let blk = human(c.block_tokens);
        let today = human(c.today_tokens);
        let left = c.time_left();

        match mode {
            0 => {
                let p = self.block_pct();
                v.ring(p, level(p));
                v.t(36.0, "CLAUDE", 2, CLAUDE);
                v.t(58.0, "5H WINDOW", 1, DIM);
                v.t(78.0, &format!("{p}%"), 4, TEXT);
                v.t(114.0, &blk, 2, CLAUDE);
                v.t(136.0, "TOKENS", 1, DIM);
                v.t(160.0, &format!("TODAY {today}  {} MSG", c.today_msgs), 1, TEXT);
                let reset = if c.block_start.is_some() { format!("RESET {left}") } else { "BLOCK IDLE".into() };
                v.t(178.0, &reset, 1, DIM);
            }
            1 => {
                let p = self.today_pct();
                let col = if p >= 90 { ALERT } else if p >= 70 { WARN } else if p >= 40 { CYAN } else { LIVE };
                v.ring(p, col);
                v.t(36.0, "CLAUDE", 2, CLAUDE);
                v.t(58.0, "TODAY USAGE", 1, DIM);
                let (sz, y) = if today.len() > 4 { (3, 84.0) } else { (4, 78.0) };
                v.t(y, &today, sz, TEXT);
                v.t(116.0, &format!("{p}% DAILY PROGRESS"), 1, CYAN);
                v.t(136.0, "TOTAL TOKENS", 1, DIM);
                v.t(160.0, &format!("MESSAGES: {} SENT", c.today_msgs), 1, TEXT);
                v.t(178.0, &format!("5H BLOCK: {blk} ({}%)", self.block_pct()), 1, DIM);
            }
            2 => {
                let p = self.timer_pct();
                let col = if p == 0 { TRACK } else if p <= 10 { ALERT } else if p <= 25 { WARN } else { CYAN };
                v.ring(p, col);
                v.t(36.0, "5H TIMER", 2, CYAN);
                v.t(58.0, "5H RESET COUNTDOWN", 1, DIM);
                let (sz, y) = if left.len() > 5 { (2, 86.0) } else { (3, 80.0) };
                v.t(y, &left, sz, if c.block_start.is_some() { CYAN } else { WARN });
                v.t(116.0, &if c.block_start.is_some() { format!("{p}% TIME REMAINING") } else { "SESSION IDLE".into() }, 1, DIM);
                v.t(136.0, "TIME REMAINING", 1, DIM);
                v.t(160.0, &format!("5H USAGE: {}% ({blk})", self.block_pct()), 1, TEXT);
                v.t(178.0, &format!("TODAY {today}  {} MSG", c.today_msgs), 1, DIM);
            }
            3 => {
                let p = self.block_pct();
                v.ring(p, level(p));
                v.t(32.0, "CLAUDE", 2, CLAUDE);
                v.t(50.0, "ALL METRICS", 1, DIM);
                v.t(68.0, &format!("5H TOK: {blk} ({p}%)"), 1, CLAUDE);
                v.t(84.0, &format!("TODAY : {today} TOKENS"), 1, TEXT);
                v.t(100.0, &format!("MSGS  : {} SENT", c.today_msgs), 1, TEXT);
                v.t(116.0, &format!("RESET : {left}"), 1, CYAN);
                v.t(134.0, "LINK  : LIVE (115200)", 1, LIVE);
            }
            MODE_CLOCK => {
                let (time, sec, date, day, extra) = self.clock();
                let [r, g, b] = self.s.clock_color;
                let accent = Color32::from_rgb(r, g, b);
                v.ring((sec * 100 / 60) as u8, accent);
                v.t(36.0, &date, 2, accent);
                v.t(58.0, &day, 1, DIM);
                v.t(78.0, &time, 4, TEXT);
                v.t(114.0, &format!(":{sec:02}"), 2, accent);
                v.t(160.0, &extra, 1, DIM);
            }
            MODE_SYSTEM => {
                let cpu = self.sys.cpu.round() as u8;
                v.ring(cpu, if cpu >= 85 { ALERT } else if cpu >= 60 { WARN } else { LIVE });
                let (ram, l1, l2) = self.sys_lines();
                let [r, g, b] = self.s.sys_color;
                v.t(36.0, "SYSTEM", 2, Color32::from_rgb(r, g, b));
                v.t(58.0, "CPU LOAD", 1, DIM);
                v.t(78.0, &format!("{cpu}%"), 4, TEXT);
                v.t(114.0, &format!("RAM {}%", self.sys.ram_pct()), 2, PURPLE);
                v.t(134.0, &ram, 1, DIM);
                v.t(160.0, &l1, 1, TEXT);
                v.t(178.0, &l2, 1, DIM);
            }
            MODE_GT => {
                // Face, needle and bars are drawn by the preview itself
                v.gauge = Some(Gauge {
                    cpu: self.sys.cpu.round() as u8,
                    gpu: self.sys.gpu.map(|g| g.round() as u8),
                    ram: self.sys.ram_pct(),
                });
            }
            _ => {
                let (title, value, sub, line, pct, color) = self.custom();
                let col = Color32::from_rgb(color[0], color[1], color[2]);
                v.ring(pct, col);
                let title = field(&title, 11);
                if title.len() <= 8 { v.t(36.0, &title, 2, col) } else { v.t(40.0, &title, 1, col) }
                v.t(58.0, &field(&sub, 18), 1, DIM);
                let value = field(&value, 8);
                if value.len() <= 5 { v.t(78.0, &value, 4, TEXT) } else { v.t(82.0, &value, 3, TEXT) }
                v.t(160.0, &field(&line, 22), 1, TEXT);
            }
        }
        v
    }
}

fn level(p: u8) -> Color32 {
    if p >= 90 { ALERT } else if p >= 75 { WARN } else { CLAUDE }
}

pub struct Txt {
    pub y: f32,
    pub text: String,
    pub size: u8,
    pub color: Color32,
}

#[derive(Clone, Copy)]
pub struct Gauge {
    pub cpu: u8,
    pub gpu: Option<u8>,
    pub ram: u8,
}

#[derive(Default)]
pub struct View {
    pub texts: Vec<Txt>,
    pub pct: u8,
    pub ring: Color32,
    /// GT widget: drawn as a tachometer instead of the ring and texts
    pub gauge: Option<Gauge>,
}

impl View {
    fn t(&mut self, y: f32, text: &str, size: u8, color: Color32) {
        self.texts.push(Txt { y, text: text.to_string(), size, color });
    }
    fn ring(&mut self, pct: u8, color: Color32) {
        self.pct = pct.min(100);
        self.ring = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data<'a>(c: &'a ClaudeStats, s: &'a SysStats, st: &'a Settings) -> Data<'a> {
        Data { claude: c, sys: s, page: st.customs.first(), cmd: None, s: st }
    }

    #[test]
    fn lines_fit_board_buffer() {
        // Worst case: every field at its maximum length
        let mut st = Settings::default();
        let page = &mut st.customs[0];
        page.title = "X".repeat(50);
        page.value = "9".repeat(50);
        page.subtitle = "S|S".repeat(20);
        page.line = "Ünïcode line that is way too long".into();
        st.clock_extra = ClockExtra::Text;
        st.clock_text = "T".repeat(60);
        let c = ClaudeStats { block_tokens: 999_999_999_999, today_tokens: u64::MAX / 2, today_msgs: 99999, ..Default::default() };
        let s = SysStats { cpu: 100.0, gpu: Some(100.0), ram_used: 64 << 30, ram_total: 64 << 30, net_rx: u64::MAX / 4, net_tx: 1, uptime: 9_999_999, disk_pct: 100 };
        let d = data(&c, &s, &st);
        assert_eq!(d.gt_line(), "G|100|100|100");
        for line in [d.claude_line(), d.clock_line(), d.system_line(), d.custom_line(), d.gt_line()] {
            assert!(line.len() < 80, "{} chars: {line}", line.len());
            assert!(line.is_ascii(), "{line}");
            assert!(!line.contains("||"), "empty field: {line}");
        }
        assert_eq!(d.custom_line().split('|').count(), 7);
        let none = SysStats { gpu: None, ..s };
        assert_eq!(data(&c, &none, &st).gt_line(), "G|100|-|100");
    }

    #[test]
    fn old_settings_load() {
        // Saved before the GT widget: 7 playlist entries
        let mut v = serde_json::to_value(Settings::default()).unwrap();
        v["playlist"] = serde_json::json!([true, true, false, false, true, true, false]);
        let s: Settings = serde_json::from_value(v).unwrap();
        assert_eq!(s.playlist.len(), 7);
        assert_eq!(s.port, "");
    }

    #[test]
    fn time_sources() {
        use crate::custom::Source;
        let now = chrono::Local::now().timestamp();
        let mut p = CustomPage { source: Source::Pomodoro, pomo_minutes: 25, ..Default::default() };
        assert_eq!(p.resolve(None).value, "25:00");
        assert_eq!(p.resolve(None).pct, 100);
        p.pomo_end = Some(now + 600);
        let r = p.resolve(None);
        assert!(r.value == "10:00" || r.value == "09:59", "{}", r.value);
        assert_eq!(r.pct, 40);
        p.pomo_end = Some(now - 1);
        assert_eq!(p.resolve(None).value, "DONE");

        let mut c = CustomPage { source: Source::Countdown, ..Default::default() };
        c.countdown_to = (chrono::Local::now() + chrono::Duration::days(3) + chrono::Duration::minutes(5))
            .format("%Y-%m-%d %H:%M").to_string();
        c.countdown_from = now - 3 * 86400;
        let r = c.resolve(None);
        assert_eq!(r.value, "3d00h");
        assert!((49..=50).contains(&r.pct), "{}", r.pct);
        c.countdown_to = "garbage".into();
        assert_eq!(c.resolve(None).value, "SET DATE");
    }

    #[test]
    fn command_output() {
        let o = CmdOutput::parse("\n 21.5C | 40 | WEATHER | sunny\n");
        assert_eq!(o.value, "21.5C");
        assert_eq!(o.pct, Some(40));
        assert_eq!(o.subtitle.as_deref(), Some("WEATHER"));
        assert_eq!(o.line.as_deref(), Some("sunny"));
        assert_eq!(CmdOutput::parse("73%").pct, Some(73));
        assert_eq!(CmdOutput::parse("1234").pct, None);
    }

    #[test]
    fn rgb565() {
        assert_eq!(to565([255, 255, 255]), 0xFFFF);
        assert_eq!(to565([0xD9, 0x77, 0x57]), 0xDBAA);
    }
}
