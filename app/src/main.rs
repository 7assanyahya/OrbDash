#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! OrbDash: drives the round GC9A01 display on the Arduino.
//! Pick which widget the screen shows, rotate through a playlist, turn the
//! screen, and feed every widget its data: Claude usage, clock, PC stats,
//! and any number of custom pages (text, command output, countdown, pomodoro).

mod claude;
mod custom;
mod link;
mod system;
mod ui;
mod widgets;

use claude::{ClaudeScanner, ClaudeStats};
use custom::{CmdOutput, CustomPage, Source};
use eframe::egui::{self, Color32, FontId, Pos2, RichText, Stroke, Vec2};
use link::{Link, LinkEvent, PortInfo};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use system::{SysMonitor, SysStats};
use widgets::*;

const SETTINGS_KEY: &str = "roundtft_settings";
/// The board takes a few seconds to boot (bootloader + intro animation)
const BOOT_TIMEOUT: Duration = Duration::from_secs(9);

const APP_NAME: &str = "OrbDash";
/// Name the app had before; its saved settings are carried over once
const OLD_APP_NAME: &str = "RoundTFT Control";

fn migrate_settings() {
    let (Some(new), Some(old)) = (eframe::storage_dir(APP_NAME), eframe::storage_dir(OLD_APP_NAME)) else {
        return;
    };
    let (new_file, old_file) = (new.join("app.ron"), old.join("app.ron"));
    if !new_file.exists() && old_file.exists() {
        let _ = std::fs::create_dir_all(&new);
        let _ = std::fs::copy(old_file, new_file);
    }
}

fn main() -> eframe::Result {
    migrate_settings();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).expect("icon.png"))
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([880.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

// ---------------------------------------------------------------------------
// Background collector: Claude logs, PC stats, custom page commands
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Shared {
    claude: ClaudeStats,
    sys: SysStats,
    /// Latest output per custom page id
    cmds: HashMap<u64, CmdOutput>,
    /// (page id, command, interval secs), written by the UI
    cmd_cfg: Vec<(u64, String, u32)>,
    claude_scanned: bool,
}

fn spawn_collector(shared: Arc<Mutex<Shared>>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let mut sys = SysMonitor::new();
        let mut scanner = ClaudeScanner::default();
        let mut last_claude: Option<Instant> = None;
        let mut last_run: HashMap<u64, (Instant, String)> = HashMap::new();
        let running: Arc<Mutex<HashSet<u64>>> = Arc::default();
        let demo = std::env::var_os("ORBDASH_DEMO").is_some();

        loop {
            let stats = sys.sample();
            shared.lock().unwrap().sys = stats;

            if last_claude.is_none_or(|t| t.elapsed() >= Duration::from_secs(5)) {
                let c = if demo { demo_claude() } else { scanner.scan() };
                let mut sh = shared.lock().unwrap();
                sh.claude = c;
                sh.claude_scanned = true;
                last_claude = Some(Instant::now());
            }

            // Each command runs on its own thread so a slow one (e.g. curl)
            // never stalls the stats
            let cfg = shared.lock().unwrap().cmd_cfg.clone();
            for (id, command, secs) in cfg {
                let due = last_run.get(&id).is_none_or(|(t, c)| {
                    t.elapsed() >= Duration::from_secs(secs.max(1) as u64) || *c != command
                });
                if command.trim().is_empty() || !due || running.lock().unwrap().contains(&id) {
                    continue;
                }
                last_run.insert(id, (Instant::now(), command.clone()));
                running.lock().unwrap().insert(id);
                let (shared, running, ctx) = (shared.clone(), running.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let out = run_command(&command);
                    shared.lock().unwrap().cmds.insert(id, out);
                    running.lock().unwrap().remove(&id);
                    ctx.request_repaint();
                });
            }

            ctx.request_repaint();
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

/// ORBDASH_DEMO=1: sample Claude numbers instead of your logs, for screenshots
fn demo_claude() -> ClaudeStats {
    ClaudeStats {
        block_tokens: 3_400_000,
        block_start: Some(chrono::Utc::now() - chrono::Duration::minutes(186)),
        today_tokens: 8_100_000,
        today_msgs: 41,
        log_files: 12,
    }
}

#[cfg(windows)]
fn shell(command: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut c = std::process::Command::new("cmd");
    // raw_arg: cmd.exe has its own quoting rules, and Rust's default escaping
    // (\") would mangle quoted arguments like curl "url"
    c.raw_arg("/C").raw_arg(command);
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flashing
    c
}

#[cfg(not(windows))]
fn shell(command: &str) -> std::process::Command {
    let mut c = std::process::Command::new("sh");
    c.args(["-c", command]);
    c
}

fn run_command(command: &str) -> CmdOutput {
    match shell(command).output() {
        Ok(o) if o.status.success() => CmdOutput::parse(&String::from_utf8_lossy(&o.stdout)),
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let err = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
            CmdOutput {
                error: Some(format!("exit {} {}", o.status.code().unwrap_or(-1), err).trim().to_string()),
                ..Default::default()
            }
        }
        Err(e) => CmdOutput { error: Some(e.to_string()), ..Default::default() },
    }
}

// ---------------------------------------------------------------------------
// Start with Windows (HKCU Run key)
// ---------------------------------------------------------------------------

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

fn reg() -> std::process::Command {
    let mut cmd = std::process::Command::new("reg");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console flashing
    }
    cmd
}

fn set_autostart(enable: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = reg();
    if enable {
        cmd.args(["add", RUN_KEY, "/v", APP_NAME, "/t", "REG_SZ", "/d"])
            .arg(format!("\"{}\"", exe.display()))
            .arg("/f");
    } else {
        cmd.args(["delete", RUN_KEY, "/v", APP_NAME, "/f"]);
    }
    // Drop the entry from before the rename, which points at the old exe
    let _ = reg().args(["delete", RUN_KEY, "/v", "RoundTFT", "/f"]).output();
    let out = cmd.output().map_err(|e| e.to_string())?;
    // Deleting a value that isn't there is fine
    if out.status.success() || !enable { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Something the screen can show: a built-in widget or one custom page
#[derive(PartialEq, Clone, Copy)]
enum Slot {
    Mode(usize),
    Page(u64),
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Dashboard,
    Widgets,
    Pages,
    Claude,
    System,
    Clock,
    Settings,
}

struct Board {
    connected_at: Instant,
    ready: bool,
    mode: usize,
    rotation: u8,
    auto_cycle: bool,
}

struct App {
    s: Settings,
    shared: Arc<Mutex<Shared>>,
    link: Option<Link>,
    board: Option<Board>,
    ports: Vec<PortInfo>,
    status: String,
    log: VecDeque<(bool, String)>,
    tab: Tab,
    /// Shown in the preview when no board is connected
    preview: Slot,
    /// Custom page loaded into the board's CUSTOM slot
    board_page: Option<u64>,
    /// Custom page being edited in the Custom tab
    edit_page: Option<u64>,
    last_fast: Instant,
    last_slow: Instant,
    last_rotate: Instant,
    last_port_scan: Instant,
    last_reconnect: Instant,
    custom_dirty: bool,
    app_error: Option<String>,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let mut s: Settings = cc
            .storage
            .and_then(|st| eframe::get_value(st, SETTINGS_KEY))
            .unwrap_or_default();
        // Settings from before a widget was added have a shorter playlist
        s.playlist.resize(MODE_NAMES.len(), false);

        ui::apply_theme(&cc.egui_ctx);
        let shared = Arc::new(Mutex::new(Shared::default()));
        spawn_collector(shared.clone(), cc.egui_ctx.clone());

        let long_ago = Instant::now() - Duration::from_secs(3600);
        let first_page = s.customs.first().map(|p| p.id);
        App {
            s,
            shared,
            link: None,
            board: None,
            ports: link::list_ports(),
            status: "Not connected".into(),
            log: VecDeque::new(),
            tab: Tab::Dashboard,
            preview: Slot::Mode(0),
            board_page: None,
            edit_page: first_page,
            last_fast: long_ago,
            last_slow: long_ago,
            last_rotate: Instant::now(),
            last_port_scan: Instant::now(),
            last_reconnect: long_ago,
            custom_dirty: false,
            app_error: None,
        }
    }

    fn push_log(&mut self, out: bool, line: String) {
        self.log.push_back((out, line));
        while self.log.len() > 300 {
            self.log.pop_front();
        }
    }

    fn send(&mut self, line: String) {
        if let Some(l) = &self.link {
            l.send(line.clone());
            self.push_log(true, line);
        }
    }

    fn board_ready(&self) -> bool {
        self.board.as_ref().is_some_and(|b| b.ready)
    }

    fn page(&self, id: u64) -> Option<&CustomPage> {
        self.s.customs.iter().find(|p| p.id == id)
    }

    /// What the screen is showing (or what the preview shows when offline)
    fn showing(&self) -> Slot {
        match self.board.as_ref().filter(|b| b.ready) {
            Some(b) if b.mode == MODE_CUSTOM => match self.board_page.or(self.s.customs.first().map(|p| p.id)) {
                Some(id) => Slot::Page(id),
                None => Slot::Mode(MODE_CUSTOM),
            },
            Some(b) => Slot::Mode(b.mode),
            None => self.preview,
        }
    }

    // ---- connection --------------------------------------------------------

    fn connect(&mut self, ctx: &egui::Context) {
        if self.s.port.is_empty() {
            if let Some(p) = self.ports.iter().find(|p| p.looks_like_arduino).or(self.ports.first()) {
                self.s.port = p.name.clone();
            }
        }
        if self.s.port.is_empty() {
            self.status = "No serial ports found".into();
            return;
        }
        match Link::open(&self.s.port, ctx.clone()) {
            Ok(l) => {
                self.status = format!("Connected to {}, waiting for the board to boot...", l.port);
                self.link = Some(l);
                self.board = Some(Board {
                    connected_at: Instant::now(),
                    ready: false,
                    mode: 0,
                    rotation: 0,
                    auto_cycle: false,
                });
                self.board_page = None;
            }
            Err(e) => self.status = format!("Can't open {}: {e}", self.s.port),
        }
        self.last_reconnect = Instant::now();
    }

    fn disconnect(&mut self, reason: &str) {
        if let Some(b) = &self.board {
            // Keep previewing what the screen last showed
            self.preview = match b.mode {
                MODE_CUSTOM => self.board_page.map(Slot::Page).unwrap_or(Slot::Mode(0)),
                m => Slot::Mode(m),
            };
        }
        self.link = None;
        self.board = None;
        self.status = reason.to_string();
    }

    fn on_board_line(&mut self, line: String) {
        let mut parts = line.split('|');
        let key = parts.next().unwrap_or("");
        let val = parts.next().and_then(|v| v.trim().parse::<usize>().ok());
        let mut just_ready = false;
        let mut need_page = false;
        if let Some(b) = &mut self.board {
            match (key, val) {
                // RTFT: firmware from before the rename
                ("ORB" | "RTFT", _) => {
                    just_ready = !b.ready;
                    b.ready = true;
                }
                ("MODE", Some(m)) => {
                    b.mode = m.min(MODE_NAMES.len() - 1);
                    need_page = b.mode == MODE_CUSTOM && self.board_page.is_none();
                }
                ("ROT", Some(r)) => b.rotation = r as u8 % 4,
                ("AUTO", Some(a)) => b.auto_cycle = a != 0,
                _ => {}
            }
        }
        self.push_log(false, line);
        if just_ready {
            self.on_ready();
        }
        if need_page {
            // Button took the board to CUSTOM: load the first page
            if let Some(id) = self.s.customs.first().map(|p| p.id) {
                self.board_page = Some(id);
                self.custom_dirty = true;
            }
        }
    }

    /// Board finished booting: push our screen settings and fresh data
    fn on_ready(&mut self) {
        if let Some(port) = self.link.as_ref().map(|l| l.port.clone()) {
            self.status = format!("Connected to {port}");
        }
        if self.s.screen_rotation != 0 {
            self.send(format!("R|{}", self.s.screen_rotation));
        }
        // Show whatever the preview was on
        let slot = self.preview;
        self.show(slot);
        self.last_fast = Instant::now() - Duration::from_secs(60);
        self.last_slow = self.last_fast;
        self.last_rotate = Instant::now();
    }

    fn show(&mut self, slot: Slot) {
        self.preview = slot;
        if let Slot::Page(id) = slot {
            self.edit_page = Some(id);
        }
        if !self.board_ready() {
            return;
        }
        let mode = match slot {
            Slot::Mode(m) => m,
            Slot::Page(_) => MODE_CUSTOM,
        };
        self.send(format!("W|{mode}"));
        if let Some(b) = &mut self.board {
            b.mode = mode;
            b.auto_cycle = false;
        }
        // Feed it right away so it doesn't wait for the next tick
        let line = match slot {
            Slot::Page(id) => {
                self.board_page = Some(id);
                self.custom_line()
            }
            Slot::Mode(m) => self.line_for(m),
        };
        self.send(line);
        self.last_rotate = Instant::now();
    }

    fn data_line<R>(&self, f: impl FnOnce(&Data) -> R) -> R {
        let sh = self.shared.lock().unwrap();
        let page = self.board_page.and_then(|id| self.page(id)).or(self.s.customs.first());
        let cmd = page.and_then(|p| sh.cmds.get(&p.id));
        let d = Data { claude: &sh.claude, sys: &sh.sys, page, cmd, s: &self.s };
        f(&d)
    }

    fn line_for(&self, mode: usize) -> String {
        self.data_line(|d| match mode {
            MODE_CLOCK => d.clock_line(),
            MODE_SYSTEM => d.system_line(),
            MODE_CUSTOM => d.custom_line(),
            MODE_GT => d.gt_line(),
            _ => d.claude_line(),
        })
    }

    fn custom_line(&self) -> String {
        self.line_for(MODE_CUSTOM)
    }

    /// Built-ins ticked in the playlist, then custom pages in rotation
    fn rotation(&self) -> Vec<Slot> {
        let mut v: Vec<Slot> = BUILTINS
            .into_iter()
            .filter(|&m| self.s.playlist.get(m).copied().unwrap_or(false))
            .map(Slot::Mode)
            .collect();
        v.extend(self.s.customs.iter().filter(|p| p.in_rotation).map(|p| Slot::Page(p.id)));
        v
    }

    // ---- per-frame logic ---------------------------------------------------

    fn tick(&mut self, ctx: &egui::Context) {
        let events = self.link.as_ref().map(|l| l.poll()).unwrap_or_default();
        for ev in events {
            match ev {
                LinkEvent::Line(l) => self.on_board_line(l),
                LinkEvent::Closed(reason) => {
                    let msg = reason.map(|r| format!("Disconnected: {r}")).unwrap_or("Disconnected".into());
                    self.disconnect(&msg);
                }
            }
        }

        {
            let cfg = self
                .s
                .customs
                .iter()
                .filter(|p| p.source == Source::Command)
                .map(|p| (p.id, p.command.clone(), p.command_secs))
                .collect();
            let mut sh = self.shared.lock().unwrap();
            sh.cmd_cfg = cfg;
            let ids: HashSet<u64> = self.s.customs.iter().map(|p| p.id).collect();
            sh.cmds.retain(|id, _| ids.contains(id));
        }

        if self.last_port_scan.elapsed() > Duration::from_secs(3) {
            self.ports = link::list_ports();
            self.last_port_scan = Instant::now();
        }

        // First run: pick the Arduino by itself
        if self.s.port.is_empty() && self.s.auto_connect {
            if let Some(p) = self.ports.iter().find(|p| p.looks_like_arduino) {
                self.s.port = p.name.clone();
            }
        }

        // Auto-reconnect when the board is plugged back in
        if self.link.is_none()
            && self.s.auto_connect
            && !self.s.port.is_empty()
            && self.last_reconnect.elapsed() > Duration::from_secs(3)
            && self.ports.iter().any(|p| p.name == self.s.port)
        {
            self.connect(ctx);
        }

        // Older firmware never says READY; assume it's up after the boot time
        if let Some(b) = &mut self.board {
            if !b.ready && b.connected_at.elapsed() > BOOT_TIMEOUT {
                b.ready = true;
                self.on_ready();
            }
        }
        if !self.board_ready() {
            return;
        }

        // Clock + system every second (and the GT gauge while it's showing),
        // Claude every 5 s. The custom page every 5 s, every second for
        // timers, and right away after an edit.
        let page_ticks = self.board_page.and_then(|id| self.page(id)).is_some_and(|p| p.source.ticks());
        let on_custom = self.board.as_ref().is_some_and(|b| b.mode == MODE_CUSTOM);
        let on_gt = self.board.as_ref().is_some_and(|b| b.mode == MODE_GT);
        if self.last_fast.elapsed() >= Duration::from_secs(1) {
            self.last_fast = Instant::now();
            let clock = self.line_for(MODE_CLOCK);
            let sys = self.line_for(MODE_SYSTEM);
            self.send(clock);
            self.send(sys);
            if on_gt {
                let gt = self.line_for(MODE_GT);
                self.send(gt);
            }
            if page_ticks && on_custom {
                self.custom_dirty = true;
            }
        }
        if self.last_slow.elapsed() >= Duration::from_secs(5) {
            self.last_slow = Instant::now();
            let u = self.line_for(0);
            self.send(u);
            self.custom_dirty = true;
        }
        if self.custom_dirty {
            self.custom_dirty = false;
            let m = self.custom_line();
            self.send(m);
        }

        // App playlist rotation
        if self.s.app_rotate && self.last_rotate.elapsed() >= Duration::from_secs(self.s.rotate_secs.max(2) as u64) {
            let seq = self.rotation();
            if !seq.is_empty() {
                let cur = self.showing();
                let next = match seq.iter().position(|s| *s == cur) {
                    Some(i) => seq[(i + 1) % seq.len()],
                    None => seq[0],
                };
                if next != cur {
                    self.show(next);
                }
            }
            self.last_rotate = Instant::now();
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick(ctx);
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui_root(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.s);
    }
}

fn rgb([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn templates_run() {
        // Quoted arguments must reach the program intact through cmd.exe
        let o = super::run_command("echo \"a|50|b\"");
        assert_eq!(o.error, None);
        assert_eq!(o.value, "\"a");
        let o = super::run_command("powershell -NoProfile -Command \"Write-Output '42|7'\"");
        assert_eq!((o.value.as_str(), o.pct), ("42", Some(7)));
    }

    /// Serve one HTTP reply on a free local port; returns "127.0.0.1:port"
    fn fake_printer(status: &'static str, body: &'static str) -> String {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = [0u8; 2048];
                let _ = s.read(&mut buf);
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(reply.as_bytes());
            }
        });
        addr
    }

    fn run_printer(template: &str, addr: &str) -> super::CmdOutput {
        let (_, page) = crate::custom::templates().into_iter().find(|(n, _)| *n == template).unwrap();
        super::run_command(&page.command.replace("PRINTER_IP", addr))
    }

    #[test]
    fn printer_templates() {
        let oct = r#"{"state":"Printing","job":{"file":{"display":"orbdash_shell.gcode"}},"progress":{"completion":62.4,"printTimeLeft":4830}}"#;
        let o = run_printer("3D printer (OctoPrint)", &fake_printer("200 OK", oct));
        assert_eq!(o.error, None);
        assert_eq!((o.value.as_str(), o.pct, o.line.as_deref()), ("62%", Some(62), Some("1h20m left")));

        let idle = r#"{"state":"Operational","job":{"file":{"display":"case.gcode"}},"progress":{"completion":null,"printTimeLeft":null}}"#;
        let o = run_printer("3D printer (OctoPrint)", &fake_printer("200 OK", idle));
        assert_eq!((o.value.as_str(), o.line.as_deref()), ("READY", Some("case.gcode")));

        let klip = r#"{"result":{"status":{"print_stats":{"state":"printing","print_duration":3600,"filename":"a.gcode"},"display_status":{"progress":0.5}}}}"#;
        let o = run_printer("3D printer (Klipper)", &fake_printer("200 OK", klip));
        assert_eq!((o.value.as_str(), o.pct, o.line.as_deref()), ("50%", Some(50), Some("1h00m left")));

        let um = r#"{"state":"printing","progress":0.25,"time_elapsed":1200,"time_total":4800,"name":"shell"}"#;
        let o = run_printer("3D printer (UltiMaker)", &fake_printer("200 OK", um));
        assert_eq!((o.value.as_str(), o.pct, o.line.as_deref()), ("25%", Some(25), Some("1h00m left")));
        let o = run_printer("3D printer (UltiMaker)", &fake_printer("404 Not Found", "{}"));
        assert_eq!(o.value, "READY");

        // Nothing listening: a clear OFF, not a command error
        let o = run_printer("3D printer (Klipper)", "127.0.0.1:9");
        assert_eq!((o.error, o.value.as_str()), (None, "OFF"));
    }
}
