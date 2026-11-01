#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! OrbDash: drives the round GC9A01 display on the Arduino.
//! Pick which widget the screen shows, rotate through a playlist, turn the
//! screen, and feed every widget its data: Claude usage, clock, PC stats,
//! and any number of custom pages (text, command output, countdown, pomodoro).

mod claude;
mod custom;
mod link;
mod system;
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
/// Number of built-in widgets before the board's CUSTOM slot
const BUILTINS: usize = MODE_CUSTOM;

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

        loop {
            let stats = sys.sample();
            shared.lock().unwrap().sys = stats;

            if last_claude.is_none_or(|t| t.elapsed() >= Duration::from_secs(5)) {
                let c = scanner.scan();
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
    Claude,
    Clock,
    System,
    Custom,
    App,
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
        let s: Settings = cc
            .storage
            .and_then(|st| eframe::get_value(st, SETTINGS_KEY))
            .unwrap_or_default();

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
            tab: Tab::Claude,
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
                    b.mode = m.min(MODE_CUSTOM);
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
            _ => d.claude_line(),
        })
    }

    fn custom_line(&self) -> String {
        self.line_for(MODE_CUSTOM)
    }

    /// Built-ins ticked in the playlist, then custom pages in rotation
    fn rotation(&self) -> Vec<Slot> {
        let mut v: Vec<Slot> = (0..BUILTINS).filter(|&m| self.s.playlist[m]).map(Slot::Mode).collect();
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

        // Clock + system every second, Claude every 5 s. The custom page every
        // 5 s, every second for timers, and right away after an edit.
        let page_ticks = self.board_page.and_then(|id| self.page(id)).is_some_and(|p| p.source.ticks());
        let on_custom = self.board.as_ref().is_some_and(|b| b.mode == MODE_CUSTOM);
        if self.last_fast.elapsed() >= Duration::from_secs(1) {
            self.last_fast = Instant::now();
            let clock = self.line_for(MODE_CLOCK);
            let sys = self.line_for(MODE_SYSTEM);
            self.send(clock);
            self.send(sys);
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
        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui));
        egui::Panel::left("widgets").exact_size(280.0).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.widget_panel(ui));
        });
        egui::Panel::right("settings").exact_size(360.0).show(ui, |ui| self.settings_panel(ui));
        egui::Panel::bottom("log").resizable(true).default_size(110.0).show(ui, |ui| self.log_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.preview(ui));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.s);
    }
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

fn rgb([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

impl App {
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("OrbDash").strong().size(18.0).color(CLAUDE));
            ui.separator();

            let connected = self.link.is_some();
            ui.add_enabled_ui(!connected, |ui| {
                let selected = if self.s.port.is_empty() { "Pick a port".to_string() } else { self.s.port.clone() };
                egui::ComboBox::from_id_salt("port")
                    .selected_text(selected)
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for p in &self.ports {
                            ui.selectable_value(&mut self.s.port, p.name.clone(), &p.label);
                        }
                        if self.ports.is_empty() {
                            ui.label("No serial ports");
                        }
                    });
            });

            if connected {
                if ui.button("Disconnect").clicked() {
                    // Stop auto-reconnect from immediately reopening it
                    self.s.auto_connect = false;
                    self.disconnect("Disconnected");
                }
            } else if ui.button("Connect").clicked() {
                self.connect(ui.ctx());
            }
            ui.checkbox(&mut self.s.auto_connect, "Auto-connect");

            ui.separator();
            let col = match &self.board {
                Some(b) if b.ready => LIVE,
                Some(_) => WARN,
                None => Color32::GRAY,
            };
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 5.0, col);
            ui.label(&self.status);
        });
        ui.add_space(4.0);
    }

    fn widget_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.heading("Widgets");
        ui.label(RichText::new("Click to show it on the screen. Tick to include it in rotation.").small().weak());
        ui.add_space(6.0);

        let showing = self.showing();
        let live = self.board_ready();
        let mut clicked = None;
        for m in 0..BUILTINS {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.s.playlist[m], "");
                if slot_button(ui, MODE_NAMES[m], showing == Slot::Mode(m), live).clicked() {
                    clicked = Some(Slot::Mode(m));
                }
            });
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Custom pages").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(page) = template_menu(ui, "+ Add") {
                    let id = self.add_page(page);
                    clicked = Some(Slot::Page(id));
                }
            });
        });
        if self.s.customs.is_empty() {
            ui.label(RichText::new("No pages yet. Add one from a template.").weak());
        }
        let mut delete = None;
        for p in &mut self.s.customs {
            ui.horizontal(|ui| {
                ui.checkbox(&mut p.in_rotation, "");
                let w = ui.available_width() - 28.0;
                let name = format!("{}  ·  {}", p.title, p.source.label());
                let resp = ui.add_sized([w, 24.0], slot_btn(&name, showing == Slot::Page(p.id), live, rgb(p.color)));
                if resp.clicked() {
                    clicked = Some(Slot::Page(p.id));
                }
                if ui.small_button("x").on_hover_text("Delete page").clicked() {
                    delete = Some(p.id);
                }
            });
        }
        if let Some(id) = delete {
            self.s.customs.retain(|p| p.id != id);
            if self.edit_page == Some(id) {
                self.edit_page = self.s.customs.first().map(|p| p.id);
            }
            if self.board_page == Some(id) {
                self.board_page = self.s.customs.first().map(|p| p.id);
                self.custom_dirty = true;
            }
            if self.preview == Slot::Page(id) {
                self.preview = Slot::Mode(0);
            }
        }
        if let Some(slot) = clicked {
            self.show(slot);
            self.tab = match slot {
                Slot::Mode(MODE_CLOCK) => Tab::Clock,
                Slot::Mode(MODE_SYSTEM) => Tab::System,
                Slot::Mode(_) => Tab::Claude,
                Slot::Page(_) => Tab::Custom,
            };
        }

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Rotation");
        let before = self.s.app_rotate;
        ui.checkbox(&mut self.s.app_rotate, "Rotate through ticked widgets and pages");
        if self.s.app_rotate && !before {
            self.last_rotate = Instant::now();
            if self.board.as_ref().is_some_and(|b| b.auto_cycle) {
                self.send("A|0".into());
            }
        }
        ui.add(egui::Slider::new(&mut self.s.rotate_secs, 3..=120).text("sec each"));

        let board_auto = self.board.as_ref().is_some_and(|b| b.auto_cycle);
        let mut auto = board_auto;
        ui.add_enabled_ui(self.board_ready(), |ui| {
            ui.checkbox(&mut auto, "Board auto-cycle (built-ins, 8 s)")
                .on_hover_text("Same as long-pressing the button on the board");
        });
        if auto != board_auto {
            self.send(format!("A|{}", auto as u8));
            if auto {
                self.s.app_rotate = false;
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Screen");
        ui.horizontal(|ui| {
            ui.label("Orientation");
            for r in 0..4u8 {
                if ui.selectable_label(self.s.screen_rotation == r, format!("{}°", r as u32 * 90)).clicked() {
                    self.s.screen_rotation = r;
                    if self.board_ready() {
                        self.send(format!("R|{r}"));
                    }
                }
            }
        });
        if let Some(b) = self.board.as_ref().filter(|b| b.ready) {
            if b.rotation != self.s.screen_rotation {
                // Board was turned with its button: follow it
                self.s.screen_rotation = b.rotation;
            }
        }
    }

    fn add_page(&mut self, mut page: CustomPage) -> u64 {
        page.id = self.s.next_id.max(1);
        self.s.next_id = page.id + 1;
        let id = page.id;
        self.s.customs.push(page);
        self.edit_page = Some(id);
        id
    }

    fn settings_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Claude, "Claude");
            ui.selectable_value(&mut self.tab, Tab::Clock, "Clock");
            ui.selectable_value(&mut self.tab, Tab::System, "System");
            ui.selectable_value(&mut self.tab, Tab::Custom, "Custom");
            ui.selectable_value(&mut self.tab, Tab::App, "App");
        });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
            Tab::Claude => self.claude_settings(ui),
            Tab::Clock => self.clock_settings(ui),
            Tab::System => self.system_settings(ui),
            Tab::Custom => self.custom_settings(ui),
            Tab::App => self.app_settings(ui),
        });
    }

    fn claude_settings(&mut self, ui: &mut egui::Ui) {
        let (c, scanned) = {
            let sh = self.shared.lock().unwrap();
            (sh.claude.clone(), sh.claude_scanned)
        };
        ui.heading("Claude usage");
        ui.label(RichText::new("Read from your local Claude Code logs.").weak());
        ui.add_space(6.0);

        egui::Grid::new("claude_stats").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
            let stat = |ui: &mut egui::Ui, k: &str, v: String| {
                ui.label(k);
                ui.label(RichText::new(v).strong().monospace());
                ui.end_row();
            };
            if !scanned {
                stat(ui, "Status", "scanning...".into());
            }
            stat(ui, "5h block", human(c.block_tokens));
            stat(ui, "Resets in", c.time_left());
            stat(ui, "Today", human(c.today_tokens));
            stat(ui, "Replies today", c.today_msgs.to_string());
            stat(ui, "Log files", c.log_files.to_string());
        });

        ui.add_space(10.0);
        ui.label("5h ring");
        let mut use_limit = self.s.block_limit_m > 0.0;
        ui.radio_value(&mut use_limit, false, "Time elapsed in the block");
        ui.radio_value(&mut use_limit, true, "Tokens vs a budget");
        if use_limit && self.s.block_limit_m <= 0.0 {
            self.s.block_limit_m = 50.0;
        } else if !use_limit {
            self.s.block_limit_m = 0.0;
        }
        if use_limit {
            ui.add(egui::Slider::new(&mut self.s.block_limit_m, 1.0..=500.0).logarithmic(true).suffix(" M tokens"));
        }

        ui.add_space(6.0);
        ui.label("Daily goal (Today ring)");
        ui.add(egui::Slider::new(&mut self.s.daily_goal_m, 0.5..=1000.0).logarithmic(true).suffix(" M tokens"));
        ui.add_space(6.0);
        ui.label(
            RichText::new("Totals include cache reads. The 5h block is estimated from your logs, not Anthropic's official limit meter.")
                .small()
                .weak(),
        );
    }

    fn clock_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Clock");
        ui.checkbox(&mut self.s.clock_24h, "24-hour time");
        ui.horizontal(|ui| {
            ui.label("Accent color");
            ui.color_edit_button_srgb(&mut self.s.clock_color);
        });
        ui.add_space(6.0);
        ui.label("Bottom line");
        ui.radio_value(&mut self.s.clock_extra, ClockExtra::Utc, "UTC time");
        ui.radio_value(&mut self.s.clock_extra, ClockExtra::Week, "Week and day of year");
        ui.radio_value(&mut self.s.clock_extra, ClockExtra::Text, "My text");
        ui.radio_value(&mut self.s.clock_extra, ClockExtra::Nothing, "Nothing");
        if self.s.clock_extra == ClockExtra::Text {
            ui.add(egui::TextEdit::singleline(&mut self.s.clock_text).char_limit(22).hint_text("up to 22 characters"));
        }
    }

    fn system_settings(&mut self, ui: &mut egui::Ui) {
        let s = self.shared.lock().unwrap().sys.clone();
        ui.heading("System");
        ui.label(RichText::new("Live stats from this PC, sent every second.").weak());
        ui.horizontal(|ui| {
            ui.label("Title color");
            ui.color_edit_button_srgb(&mut self.s.sys_color);
        });
        ui.add_space(6.0);
        egui::Grid::new("sys_stats").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
            let mut stat = |k: &str, v: String| {
                ui.label(k);
                ui.label(RichText::new(v).strong().monospace());
                ui.end_row();
            };
            stat("CPU", format!("{:.0}%", s.cpu));
            stat("RAM", format!("{}% ({:.1} / {:.1} GB)", s.ram_pct(), s.ram_used as f64 / 1e9, s.ram_total as f64 / 1e9));
            stat("Network", format!("down {}/s  up {}/s", human(s.net_rx), human(s.net_tx)));
            stat("Disk C:", format!("{}% used", s.disk_pct));
            stat("Uptime", format!("{}d {}h {}m", s.uptime / 86400, s.uptime / 3600 % 24, s.uptime / 60 % 60));
        });
    }

    fn custom_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Custom pages");
        ui.label(RichText::new("Make as many as you like; they share the board's CUSTOM slot.").weak());
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            let current = self.edit_page.and_then(|id| self.page(id)).map(|p| p.title.clone());
            egui::ComboBox::from_id_salt("edit_page")
                .selected_text(current.unwrap_or("Pick a page".into()))
                .width(170.0)
                .show_ui(ui, |ui| {
                    for p in &self.s.customs {
                        ui.selectable_value(&mut self.edit_page, Some(p.id), &p.title);
                    }
                });
            if let Some(page) = template_menu(ui, "+ Add page") {
                let id = self.add_page(page);
                self.show(Slot::Page(id));
            }
        });
        ui.separator();

        let Some(idx) = self.edit_page.and_then(|id| self.s.customs.iter().position(|p| p.id == id)) else {
            ui.label("Pick or add a page.");
            return;
        };

        let cmd_out = self.shared.lock().unwrap().cmds.get(&self.s.customs[idx].id).cloned();
        let before = serde_json::to_string(&self.s.customs[idx]).unwrap_or_default();
        let now = chrono::Local::now().timestamp();
        let p = &mut self.s.customs[idx];

        egui::Grid::new("custom").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Title");
            ui.add(egui::TextEdit::singleline(&mut p.title).char_limit(11));
            ui.end_row();
            ui.label("Subtitle");
            ui.add(egui::TextEdit::singleline(&mut p.subtitle).char_limit(18));
            ui.end_row();
            ui.label("Bottom line");
            ui.add(egui::TextEdit::singleline(&mut p.line).char_limit(22));
            ui.end_row();
            ui.label("Color");
            ui.color_edit_button_srgb(&mut p.color);
            ui.end_row();
            ui.label("Source");
            egui::ComboBox::from_id_salt("source").selected_text(p.source.label()).show_ui(ui, |ui| {
                for s in Source::ALL {
                    ui.selectable_value(&mut p.source, s, s.label());
                }
            });
            ui.end_row();
        });
        ui.add_space(6.0);

        match p.source {
            Source::Static => {
                egui::Grid::new("static").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label("Value");
                    ui.add(egui::TextEdit::singleline(&mut p.value).char_limit(8));
                    ui.end_row();
                    ui.label("Ring %");
                    ui.add(egui::Slider::new(&mut p.pct, 0..=100));
                    ui.end_row();
                });
            }
            Source::Command => {
                ui.add(
                    egui::TextEdit::singleline(&mut p.command)
                        .hint_text("any command, e.g. curl -s wttr.in/?format=%t")
                        .desired_width(f32::INFINITY),
                );
                ui.add(egui::Slider::new(&mut p.command_secs, 1..=3600).logarithmic(true).text("sec between runs"));
                ui.label(
                    RichText::new("First output line: value|ring%|subtitle|bottom line. Only value is required; a bare number 0-100 also fills the ring.")
                        .small()
                        .weak(),
                );
                match &cmd_out {
                    Some(o) if o.error.is_some() => {
                        ui.colored_label(ALERT, format!("Command failed: {}", o.error.clone().unwrap_or_default()));
                    }
                    Some(o) => {
                        ui.label(RichText::new(format!("Last output: {}", o.value)).monospace());
                    }
                    None if !p.command.trim().is_empty() => {
                        ui.label(RichText::new("Running...").weak());
                    }
                    None => {}
                }
            }
            Source::Countdown => {
                ui.label("Count down to (YYYY-MM-DD HH:MM)");
                let resp = ui.add(egui::TextEdit::singleline(&mut p.countdown_to).hint_text("2026-12-25 00:00"));
                if resp.changed() {
                    // Ring measures progress from when the date was set
                    p.countdown_from = now;
                }
                if custom::parse_when(&p.countdown_to).is_none() {
                    ui.colored_label(WARN, "Not a valid date yet");
                }
                ui.label(RichText::new("The ring fills up as the date gets closer.").small().weak());
            }
            Source::Pomodoro => {
                ui.add(egui::Slider::new(&mut p.pomo_minutes, 1..=120).text("minutes"));
                ui.horizontal(|ui| {
                    let running = p.pomo_end.is_some();
                    if running {
                        if ui.button("Pause").clicked() {
                            p.pomo_paused = Some(p.pomo_left(now).max(0));
                            p.pomo_end = None;
                        }
                    } else {
                        let label = if p.pomo_paused.is_some() { "Resume" } else { "Start" };
                        if ui.button(label).clicked() {
                            p.pomo_end = Some(now + p.pomo_left(now));
                            p.pomo_paused = None;
                        }
                    }
                    if ui.button("Reset").clicked() {
                        p.pomo_end = None;
                        p.pomo_paused = None;
                    }
                });
                ui.label(RichText::new("The ring drains as time runs out.").small().weak());
            }
        }

        let page_id = p.id;
        let after = serde_json::to_string(&self.s.customs[idx]).unwrap_or_default();
        if before != after && self.board_page == Some(page_id) {
            self.custom_dirty = true;
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Show on screen").clicked() {
                self.show(Slot::Page(page_id));
            }
        });
    }

    fn app_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("App");
        let mut autostart = self.s.start_with_windows;
        if ui.checkbox(&mut autostart, "Start with Windows").changed() {
            match set_autostart(autostart) {
                Ok(()) => {
                    self.s.start_with_windows = autostart;
                    self.app_error = None;
                }
                Err(e) => self.app_error = Some(e),
            }
        }
        if let Some(e) = &self.app_error {
            ui.colored_label(ALERT, e);
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Settings are saved automatically.").small().weak());
        ui.add_space(10.0);
        if ui.button("Reset all settings").clicked() {
            let port = std::mem::take(&mut self.s.port);
            self.s = Settings { port, start_with_windows: self.s.start_with_windows, ..Settings::default() };
            self.edit_page = self.s.customs.first().map(|p| p.id);
            self.board_page = None;
            self.preview = Slot::Mode(0);
        }
    }

    fn log_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Serial log").strong());
            ui.label(RichText::new("-> sent   <- from board").small().weak());
            if ui.small_button("Clear").clicked() {
                self.log.clear();
            }
        });
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
            for (out, line) in &self.log {
                let (arrow, col) = if *out { ("->", Color32::GRAY) } else { ("<-", CYAN) };
                ui.label(RichText::new(format!("{arrow} {line}")).monospace().small().color(col));
            }
        });
    }

    /// Round preview of what the screen is showing
    fn preview(&mut self, ui: &mut egui::Ui) {
        let slot = self.showing();
        let (view, name) = {
            let sh = self.shared.lock().unwrap();
            let (mode, page, name) = match slot {
                Slot::Mode(m) => (m, self.s.customs.first(), MODE_NAMES[m].to_string()),
                Slot::Page(id) => {
                    let p = self.page(id);
                    (MODE_CUSTOM, p, p.map(|p| p.title.clone()).unwrap_or_default())
                }
            };
            let cmd = page.and_then(|p| sh.cmds.get(&p.id));
            let d = Data { claude: &sh.claude, sys: &sh.sys, page, cmd, s: &self.s };
            (d.view(mode), name)
        };

        ui.vertical_centered(|ui| {
            let tag = if self.board_ready() { "On screen" } else { "Preview" };
            ui.label(RichText::new(format!("{tag}: {name}")).strong());
        });
        let avail = ui.available_size();
        let size = avail.x.min(avail.y - 10.0).clamp(160.0, 520.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(avail.x, size), egui::Sense::hover());
        let k = size / 240.0;
        let origin = Pos2::new(rect.center().x - size / 2.0, rect.top());
        let at = |x: f32, y: f32| Pos2::new(origin.x + x * k, origin.y + y * k);
        let p = ui.painter();

        let center = at(120.0, 120.0);
        p.circle_filled(center, 121.0 * k, Color32::from_gray(40));
        p.circle_filled(center, 120.0 * k, Color32::BLACK);
        p.circle_stroke(center, 118.0 * k, Stroke::new(1.0, TRACK));

        // 270° ring opening at the bottom, like the firmware's drawArc
        let arc = |from: f32, to: f32| -> Vec<Pos2> {
            let steps = ((to - from) / 2.0).ceil().max(1.0) as usize;
            (0..=steps)
                .map(|i| {
                    let deg = 135.0 + from + (to - from) * i as f32 / steps as f32;
                    let r = deg.to_radians();
                    at(120.0 + r.cos() * 104.0, 120.0 + r.sin() * 104.0)
                })
                .collect()
        };
        p.add(egui::Shape::line(arc(0.0, 270.0), Stroke::new(12.0 * k, TRACK)));
        if view.pct > 0 {
            p.add(egui::Shape::line(arc(0.0, 270.0 * view.pct as f32 / 100.0), Stroke::new(12.0 * k, view.ring)));
        }

        for t in &view.texts {
            // Adafruit GFX font: 6x8 px per char at size 1
            let font = FontId::monospace(8.0 * t.size as f32 * k);
            p.text(at(120.0, t.y), egui::Align2::CENTER_TOP, &t.text, font, t.color);
        }

        let tag = match &self.board {
            Some(b) if b.ready => format!("LIVE [{}/7]", b.mode + 1),
            _ => "PREVIEW".into(),
        };
        p.text(at(120.0, 200.0), egui::Align2::CENTER_TOP, tag, FontId::monospace(8.0 * k), DIM);
    }
}

fn slot_btn(label: &str, on_screen: bool, live: bool, accent: Color32) -> egui::Button<'static> {
    let text = if on_screen && live { format!("> {label} <") } else { label.to_string() };
    let mut rt = RichText::new(text).strong();
    if accent != Color32::TRANSPARENT {
        rt = rt.color(accent);
    }
    egui::Button::selectable(on_screen, rt)
}

fn slot_button(ui: &mut egui::Ui, label: &str, on_screen: bool, live: bool) -> egui::Response {
    ui.add_sized([ui.available_width(), 24.0], slot_btn(label, on_screen, live, Color32::TRANSPARENT))
}

/// "+ Add" dropdown of page templates; returns the picked one
fn template_menu(ui: &mut egui::Ui, label: &str) -> Option<CustomPage> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(("templates", label)).selected_text(label).width(110.0).show_ui(ui, |ui| {
        for (name, page) in custom::templates() {
            if ui.selectable_label(false, name).clicked() {
                picked = Some(page);
            }
        }
    });
    picked
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
}
