//! The app's look: a dark, card-based layout with a sidebar of pages.
//! All the app logic stays in main.rs; this only draws it and turns clicks
//! into the same calls (show, send, connect).

use super::*;
use eframe::egui::{Align, Align2, CornerRadius, CursorIcon, Frame, Layout, Margin, Rect, Response, Sense, Ui, vec2};

// ---------------------------------------------------------------------------
// Theme: the brand palette (brand/README.md)
// ---------------------------------------------------------------------------

const VERSION: &str = env!("CARGO_PKG_VERSION");
const EMBER: Color32 = Color32::from_rgb(255, 122, 69);
const BG: Color32 = Color32::from_rgb(13, 14, 18);
const SIDEBAR: Color32 = Color32::from_rgb(18, 19, 24);
const CARD: Color32 = Color32::from_rgb(24, 26, 32);
const CARD_HI: Color32 = Color32::from_rgb(31, 34, 42);
const BORDER: Color32 = Color32::from_rgb(38, 42, 51);
const TEXT_HI: Color32 = Color32::from_rgb(241, 242, 245);
const TEXT_MID: Color32 = Color32::from_rgb(163, 168, 182);
const TEXT_LO: Color32 = Color32::from_rgb(107, 112, 128);
const ACCENT: Color32 = Color32::from_rgb(139, 92, 246);
const OK: Color32 = Color32::from_rgb(34, 197, 94);
const AMBER: Color32 = Color32::from_rgb(245, 158, 11);
const BAD: Color32 = Color32::from_rgb(239, 68, 68);

pub fn apply_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|s| {
        use egui::{FontFamily::*, TextStyle};
        s.text_styles = [
            (TextStyle::Heading, FontId::new(20.0, Proportional)),
            (TextStyle::Body, FontId::new(14.0, Proportional)),
            (TextStyle::Button, FontId::new(14.0, Proportional)),
            (TextStyle::Small, FontId::new(12.0, Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, Monospace)),
        ]
        .into();
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(12.0, 6.0);
        s.spacing.interact_size.y = 28.0;
        s.spacing.slider_width = 220.0;

        let v = &mut s.visuals;
        v.dark_mode = true;
        v.panel_fill = BG;
        v.window_fill = CARD;
        v.extreme_bg_color = Color32::from_rgb(10, 11, 14);
        v.faint_bg_color = CARD_HI;
        v.slider_trailing_fill = true;
        v.selection.bg_fill = ACCENT.gamma_multiply(0.55);
        v.selection.stroke = Stroke::new(1.0, TEXT_HI);
        v.hyperlink_color = ACCENT;
        let r = CornerRadius::same(8);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = CARD;
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_MID);
        w.noninteractive.corner_radius = r;
        for (st, fill, stroke) in [
            (&mut w.inactive, Color32::from_rgb(38, 41, 51), BORDER),
            (&mut w.hovered, Color32::from_rgb(48, 52, 64), Color32::from_rgb(64, 68, 82)),
            (&mut w.active, Color32::from_rgb(52, 40, 86), ACCENT),
            (&mut w.open, Color32::from_rgb(40, 44, 54), Color32::from_rgb(64, 68, 82)),
        ] {
            st.bg_fill = fill;
            st.weak_bg_fill = fill;
            st.bg_stroke = Stroke::new(1.0, stroke);
            st.fg_stroke = Stroke::new(1.5, TEXT_HI);
            st.corner_radius = r;
        }
    });
}

// ---------------------------------------------------------------------------
// Building blocks
// ---------------------------------------------------------------------------

fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

fn card_title(ui: &mut Ui, title: &str, sub: &str) {
    ui.label(RichText::new(title).size(15.0).strong().color(TEXT_HI));
    if !sub.is_empty() {
        ui.label(RichText::new(sub).size(12.5).color(TEXT_LO));
    }
    ui.add_space(4.0);
}

fn label_lo(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(12.5).color(TEXT_LO));
}

/// On/off switch
fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let p = ui.painter();
    let off = Color32::from_rgb(55, 59, 70);
    let bg = Color32::from_rgb(
        (off.r() as f32 + (ACCENT.r() as f32 - off.r() as f32) * t) as u8,
        (off.g() as f32 + (ACCENT.g() as f32 - off.g() as f32) * t) as u8,
        (off.b() as f32 + (ACCENT.b() as f32 - off.b() as f32) * t) as u8,
    );
    p.rect_filled(rect, CornerRadius::same(11), bg);
    let x = rect.left() + 11.0 + (rect.width() - 22.0) * t;
    p.circle_filled(Pos2::new(x, rect.center().y), 8.0, Color32::WHITE);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Toggle with a label on the left, pushed to the right edge
fn toggle_row(ui: &mut Ui, label: &str, sub: &str, on: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(label).color(TEXT_HI));
            if !sub.is_empty() {
                label_lo(ui, sub);
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            changed = toggle(ui, on).changed();
        });
    });
    changed
}

fn accent_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(ACCENT))
        .on_hover_cursor(CursorIcon::PointingHand)
}

fn dot(ui: &mut Ui, color: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 4.5, color);
}

/// Small stat card: label, big value, a line under it, optional bar
fn stat_tile(ui: &mut Ui, label: &str, value: &str, sub: &str, frac: Option<f32>, color: Color32) {
    card(ui, |ui| {
        ui.label(RichText::new(label.to_uppercase()).size(11.5).color(TEXT_LO).strong());
        ui.label(RichText::new(value).size(26.0).strong().color(TEXT_HI));
        label_lo(ui, if sub.is_empty() { " " } else { sub });
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 5.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::same(3), Color32::from_rgb(40, 43, 52));
        if let Some(f) = frac {
            let mut fill = r;
            fill.set_width(r.width() * f.clamp(0.0, 1.0));
            p.rect_filled(fill, CornerRadius::same(3), color);
        }
    });
}

fn tiles(ui: &mut Ui, list: Vec<(&str, String, String, Option<f32>, Color32)>) {
    let n = list.len();
    ui.columns(n, |cols| {
        for (col, (label, value, sub, frac, color)) in cols.iter_mut().zip(list) {
            stat_tile(col, label, &value, &sub, frac, color);
        }
    });
}

fn level_color(p: f32) -> Color32 {
    if p >= 85.0 { BAD } else if p >= 60.0 { AMBER } else { OK }
}

/// Accent color and one-line description of each built-in widget
fn widget_info(m: usize) -> (Color32, &'static str) {
    match m {
        0 => (CLAUDE, "Your 5-hour usage window"),
        1 => (CLAUDE, "Today's tokens vs your daily goal"),
        2 => (CLAUDE, "Time until the 5-hour block resets"),
        3 => (CLAUDE, "Every Claude number on one screen"),
        MODE_CLOCK => (CYAN, "Time, date and a seconds ring"),
        MODE_SYSTEM => (LIVE, "CPU, RAM, network and uptime"),
        MODE_GT => (Color32::from_rgb(230, 210, 0), "Race-car tachometer: CPU, GPU, RAM"),
        _ => (PURPLE, "Your custom pages"),
    }
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

const PAGES: [(Tab, &str, &str); 7] = [
    (Tab::Dashboard, "🏠", "Dashboard"),
    (Tab::Widgets, "🔲", "Widgets"),
    (Tab::Pages, "📝", "Custom pages"),
    (Tab::Claude, "✳", "Claude"),
    (Tab::System, "🖥", "System"),
    (Tab::Clock, "🕐", "Clock"),
    (Tab::Settings, "⚙", "Settings"),
];

fn nav_item(ui: &mut Ui, icon: &str, label: &str, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(8), ACCENT.gamma_multiply(0.16));
        let bar = Rect::from_min_size(rect.min + vec2(0.0, 9.0), vec2(3.0, 20.0));
        p.rect_filled(bar, CornerRadius::same(2), ACCENT);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(8), CARD_HI);
    }
    let col = if selected { TEXT_HI } else { TEXT_MID };
    p.text(rect.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, icon, FontId::proportional(16.0), col);
    p.text(rect.left_center() + vec2(44.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.5), col);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

impl App {
    pub fn ui_root(&mut self, ui: &mut Ui) {
        egui::Panel::left("nav")
            .exact_size(224.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(SIDEBAR).inner_margin(Margin::same(14)))
            .show(ui, |ui| self.sidebar(ui));
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin::symmetric(28, 22)))
            .show(ui, |ui| {
                self.header(ui);
                ui.add_space(10.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                    Tab::Dashboard => self.dashboard(ui),
                    Tab::Widgets => self.widgets_page(ui),
                    Tab::Pages => self.pages_page(ui),
                    Tab::Claude => self.claude_page(ui),
                    Tab::System => self.system_page(ui),
                    Tab::Clock => self.clock_page(ui),
                    Tab::Settings => self.settings_page(ui),
                });
            });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
            draw_mark(ui.painter(), r.center(), 32.0);
            ui.label(RichText::new("Orb").size(20.0).strong().color(TEXT_HI));
            ui.add_space(-ui.spacing().item_spacing.x);
            ui.label(RichText::new("Dash").size(20.0).strong().color(ACCENT));
        });
        ui.label(RichText::new(format!("v{VERSION}")).size(11.5).color(TEXT_LO));
        ui.add_space(18.0);

        for (tab, icon, label) in PAGES {
            if nav_item(ui, icon, label, self.tab == tab).clicked() {
                self.tab = tab;
            }
        }

        // Device box pinned to the bottom of the sidebar
        ui.add_space((ui.available_height() - 124.0).max(12.0));
        {
            Frame::new()
                .fill(CARD)
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let (col, state) = match &self.board {
                        Some(b) if b.ready => (OK, "Connected"),
                        Some(_) => (AMBER, "Board starting..."),
                        None => (TEXT_LO, "Not connected"),
                    };
                    ui.horizontal(|ui| {
                        dot(ui, col);
                        ui.label(RichText::new(state).strong().color(TEXT_HI));
                    });
                    let port = if self.s.port.is_empty() { "No port picked".to_string() } else { self.s.port.clone() };
                    let fw = match self.board.as_ref().filter(|b| b.ready) {
                        Some(b) => format!("  ·  firmware {}", b.firmware.as_deref().unwrap_or("before 1.1")),
                        None => String::new(),
                    };
                    label_lo(ui, &format!("{port}{fw}"));
                    ui.add_space(2.0);
                    if self.link.is_some() {
                        if ui.add_sized([ui.available_width(), 28.0], egui::Button::new("Disconnect")).clicked() {
                            // Stop auto-reconnect from immediately reopening it
                            self.s.auto_connect = false;
                            self.disconnect("Disconnected");
                        }
                    } else if ui
                        .add_sized([ui.available_width(), 28.0], egui::Button::new(RichText::new("Connect").color(Color32::WHITE).strong()).fill(ACCENT))
                        .clicked()
                    {
                        self.connect(ui.ctx());
                    }
                });
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        let (title, sub) = match self.tab {
            Tab::Dashboard => ("Dashboard", "What your OrbDash is showing, and your PC at a glance"),
            Tab::Widgets => ("Widgets", "Pick what the screen shows and what it rotates through"),
            Tab::Pages => ("Custom pages", "Your own widgets: text, commands, countdowns, timers, printers"),
            Tab::Claude => ("Claude", "Claude Code usage, read from your local logs"),
            Tab::System => ("System", "Live stats from this PC"),
            Tab::Clock => ("Clock", "How the clock widget looks"),
            Tab::Settings => ("Settings", "Connection, startup and the serial log"),
        };
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(26.0).strong().color(TEXT_HI));
                ui.label(RichText::new(sub).color(TEXT_MID));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let name = self.slot_name(self.showing());
                Frame::new()
                    .fill(CARD)
                    .stroke(Stroke::new(1.0, BORDER))
                    .corner_radius(CornerRadius::same(20))
                    .inner_margin(Margin::symmetric(14, 7))
                    .show(ui, |ui| {
                        // Laid out right to left, so added last-first
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(name).strong().color(TEXT_HI));
                            let tag = if self.board_ready() { "On screen" } else { "Preview" };
                            ui.label(RichText::new(tag).color(TEXT_LO));
                            dot(ui, if self.board_ready() { OK } else { TEXT_LO });
                        });
                    });
            });
        });
    }

    fn slot_name(&self, slot: Slot) -> String {
        match slot {
            Slot::Mode(m) => MODE_NAMES[m].to_string(),
            Slot::Page(id) => self.page(id).map(|p| p.title.clone()).unwrap_or_default(),
        }
    }

    // ---- Dashboard ---------------------------------------------------------

    fn dashboard(&mut self, ui: &mut Ui) {
        let (sys, claude) = {
            let sh = self.shared.lock().unwrap();
            (sh.sys.clone(), sh.claude.clone())
        };
        let block = self.data_line(|d| d.block_pct());

        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(340.0, 0.0), Layout::top_down(Align::Center), |ui| {
                card(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        self.screen(ui, 290.0);
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            label_lo(ui, "Turn");
                            for r in 0..4u8 {
                                if ui.selectable_label(self.s.screen_rotation == r, format!("{}°", r as u32 * 90)).clicked() {
                                    self.set_rotation(r);
                                }
                            }
                        });
                    });
                });
            });
            ui.vertical(|ui| {
                let cpu = sys.cpu;
                let ram = sys.ram_pct() as f32;
                tiles(
                    ui,
                    vec![
                        ("CPU", format!("{cpu:.0}%"), "load".into(), Some(cpu / 100.0), level_color(cpu)),
                        (
                            "GPU",
                            sys.gpu.map(|g| format!("{g:.0}%")).unwrap_or("--".into()),
                            if sys.gpu.is_some() { "load".into() } else { "no reading".into() },
                            sys.gpu.map(|g| g / 100.0),
                            level_color(sys.gpu.unwrap_or(0.0)),
                        ),
                        (
                            "RAM",
                            format!("{ram:.0}%"),
                            format!("{:.1} / {:.0} GB", sys.ram_used as f64 / 1e9, sys.ram_total as f64 / 1e9),
                            Some(ram / 100.0),
                            level_color(ram),
                        ),
                    ],
                );
                tiles(
                    ui,
                    vec![
                        ("Claude 5h", human(claude.block_tokens), format!("resets in {}", claude.time_left()), Some(block as f32 / 100.0), CLAUDE),
                        ("Claude today", human(claude.today_tokens), format!("{} replies", claude.today_msgs), None, CLAUDE),
                    ],
                );
                card(ui, |ui| {
                    card_title(ui, "Rotation", "");
                    let before = self.s.app_rotate;
                    let secs = self.s.rotate_secs;
                    toggle_row(ui, "Rotate widgets", &format!("Every {secs} s through the ones switched on in Widgets"), &mut self.s.app_rotate);
                    if self.s.app_rotate && !before {
                        self.last_rotate = Instant::now();
                        if self.board.as_ref().is_some_and(|b| b.auto_cycle) {
                            self.send("A|0".into());
                        }
                    }
                });
            });
        });

        ui.add_space(6.0);
        card(ui, |ui| {
            card_title(ui, "Show on screen", "Click a widget to put it on the display");
            self.widget_grid(ui);
        });
    }

    /// Clickable tiles for every built-in widget and custom page
    fn widget_grid(&mut self, ui: &mut Ui) {
        let showing = self.showing();
        let live = self.board_ready();
        let mut items: Vec<(Slot, String, Color32)> =
            BUILTINS.iter().map(|&m| (Slot::Mode(m), MODE_NAMES[m].to_string(), widget_info(m).0)).collect();
        items.extend(self.s.customs.iter().map(|p| (Slot::Page(p.id), p.title.clone(), rgb(p.color))));

        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            for (slot, name, color) in items {
                let on = slot == showing;
                let (rect, resp) = ui.allocate_exact_size(vec2(158.0, 62.0), Sense::click());
                let p = ui.painter();
                let fill = if on { ACCENT.gamma_multiply(0.16) } else if resp.hovered() { CARD_HI } else { Color32::from_rgb(20, 22, 28) };
                p.rect_filled(rect, CornerRadius::same(10), fill);
                let stroke = if on { ACCENT } else { BORDER };
                p.rect_stroke(rect, CornerRadius::same(10), Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
                p.circle_filled(rect.left_top() + vec2(18.0, 21.0), 5.0, color);
                p.text(rect.left_top() + vec2(30.0, 21.0), Align2::LEFT_CENTER, &name, FontId::proportional(14.0), TEXT_HI);
                let sub = if on && live { "On screen" } else if on { "Selected" } else { "Click to show" };
                p.text(rect.left_top() + vec2(30.0, 42.0), Align2::LEFT_CENTER, sub, FontId::proportional(12.0), if on { ACCENT } else { TEXT_LO });
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    clicked = Some(slot);
                }
            }
        });
        if let Some(slot) = clicked {
            self.show(slot);
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

    fn set_rotation(&mut self, r: u8) {
        self.s.screen_rotation = r;
        if self.board_ready() {
            self.send(format!("R|{r}"));
        }
    }

    // ---- Widgets -----------------------------------------------------------

    fn widgets_page(&mut self, ui: &mut Ui) {
        let showing = self.showing();
        let mut clicked = None;
        card(ui, |ui| {
            card_title(ui, "Built-in widgets", "Switch on the ones to include in rotation");
            for (i, m) in BUILTINS.into_iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                let (color, desc) = widget_info(m);
                ui.horizontal(|ui| {
                    dot(ui, color);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(MODE_NAMES[m]).strong().color(TEXT_HI));
                        label_lo(ui, desc);
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        toggle(ui, &mut self.s.playlist[m]).on_hover_text("Include in rotation");
                        ui.add_space(8.0);
                        let on = showing == Slot::Mode(m);
                        let btn = if on { egui::Button::new(RichText::new("Showing").color(ACCENT)) } else { egui::Button::new("Show") };
                        if ui.add_enabled(!on, btn).clicked() {
                            clicked = Some(Slot::Mode(m));
                        }
                    });
                });
            }
        });
        if let Some(slot) = clicked {
            self.show(slot);
        }

        ui.add_space(6.0);
        ui.columns(2, |cols| {
            card(&mut cols[0], |ui| {
                card_title(ui, "Rotation", "Move through the widgets that are switched on");
                let before = self.s.app_rotate;
                toggle_row(ui, "Rotate from the app", "Widgets and custom pages that are on", &mut self.s.app_rotate);
                if self.s.app_rotate && !before {
                    self.last_rotate = Instant::now();
                    if self.board.as_ref().is_some_and(|b| b.auto_cycle) {
                        self.send("A|0".into());
                    }
                }
                ui.add(egui::Slider::new(&mut self.s.rotate_secs, 3..=120).suffix(" s each"));
                ui.add_space(4.0);
                let board_auto = self.board.as_ref().is_some_and(|b| b.auto_cycle);
                let mut auto = board_auto;
                ui.add_enabled_ui(self.board_ready(), |ui| {
                    toggle_row(ui, "Board auto-cycle", "Built-ins, 8 s each. Same as a long press", &mut auto);
                });
                if auto != board_auto {
                    self.send(format!("A|{}", auto as u8));
                    if auto {
                        self.s.app_rotate = false;
                    }
                }
            });
            card(&mut cols[1], |ui| {
                card_title(ui, "Screen", "Which way up the display is mounted");
                ui.horizontal(|ui| {
                    for r in 0..4u8 {
                        let text = RichText::new(format!("{}°", r as u32 * 90)).size(15.0);
                        if ui.add_sized([56.0, 34.0], egui::Button::selectable(self.s.screen_rotation == r, text)).clicked() {
                            self.set_rotation(r);
                        }
                    }
                });
                label_lo(ui, "Two taps on the board's button turn it too.");
            });
        });
        if let Some(b) = self.board.as_ref().filter(|b| b.ready) {
            if b.rotation != self.s.screen_rotation {
                // Board was turned with its button: follow it
                self.s.screen_rotation = b.rotation;
            }
        }
    }

    // ---- Custom pages ------------------------------------------------------

    fn pages_page(&mut self, ui: &mut Ui) {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(300.0, 0.0), Layout::top_down(Align::Min), |ui| {
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Your pages").size(15.0).strong().color(TEXT_HI));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let Some(page) = template_menu(ui, "+ New") {
                                let id = self.add_page(page);
                                self.show(Slot::Page(id));
                            }
                        });
                    });
                    label_lo(ui, "They take turns in the board's Custom slot");
                    ui.add_space(4.0);
                    if self.s.customs.is_empty() {
                        label_lo(ui, "No pages yet. Add one with + New.");
                    }
                    self.page_list(ui);
                });
            });
            ui.vertical(|ui| {
                card(ui, |ui| self.page_editor(ui));
            });
        });
    }

    fn page_list(&mut self, ui: &mut Ui) {
        let mut delete = None;
        let mut select = None;
        for p in &mut self.s.customs {
            let selected = self.edit_page == Some(p.id);
            Frame::new()
                .fill(if selected { ACCENT.gamma_multiply(0.14) } else { Color32::from_rgb(20, 22, 28) })
                .stroke(Stroke::new(1.0, if selected { ACCENT } else { BORDER }))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        dot(ui, rgb(p.color));
                        let resp = ui
                            .add(egui::Label::new(RichText::new(&p.title).strong().color(TEXT_HI)).sense(Sense::click()))
                            .on_hover_cursor(CursorIcon::PointingHand);
                        if resp.clicked() {
                            select = Some(p.id);
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("🗑").on_hover_text("Delete page").clicked() {
                                delete = Some(p.id);
                            }
                            toggle(ui, &mut p.in_rotation).on_hover_text("Include in rotation");
                        });
                    });
                    label_lo(ui, p.source.label());
                });
        }
        if let Some(id) = select {
            self.edit_page = Some(id);
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
    }

    fn page_editor(&mut self, ui: &mut Ui) {
        let Some(idx) = self.edit_page.and_then(|id| self.s.customs.iter().position(|p| p.id == id)) else {
            card_title(ui, "Edit page", "Pick a page on the left, or make one with + New");
            return;
        };

        let cmd_out = self.shared.lock().unwrap().cmds.get(&self.s.customs[idx].id).cloned();
        let before = serde_json::to_string(&self.s.customs[idx]).unwrap_or_default();
        let now = chrono::Local::now().timestamp();
        let p = &mut self.s.customs[idx];
        let page_id = p.id;

        card_title(ui, &format!("Edit: {}", p.title), "Changes go to the screen right away");
        egui::Grid::new("custom").num_columns(2).spacing([14.0, 10.0]).show(ui, |ui| {
            ui.label("Title");
            ui.add(egui::TextEdit::singleline(&mut p.title).char_limit(11).desired_width(260.0));
            ui.end_row();
            ui.label("Subtitle");
            ui.add(egui::TextEdit::singleline(&mut p.subtitle).char_limit(18).desired_width(260.0));
            ui.end_row();
            ui.label("Bottom line");
            ui.add(egui::TextEdit::singleline(&mut p.line).char_limit(22).desired_width(260.0));
            ui.end_row();
            ui.label("Color");
            ui.color_edit_button_srgb(&mut p.color);
            ui.end_row();
            ui.label("Source");
            egui::ComboBox::from_id_salt("source").selected_text(p.source.label()).width(260.0).show_ui(ui, |ui| {
                for s in Source::ALL {
                    ui.selectable_value(&mut p.source, s, s.label());
                }
            });
            ui.end_row();
        });
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        match p.source {
            Source::Static => {
                egui::Grid::new("static").num_columns(2).spacing([14.0, 10.0]).show(ui, |ui| {
                    ui.label("Value");
                    ui.add(egui::TextEdit::singleline(&mut p.value).char_limit(8).desired_width(260.0));
                    ui.end_row();
                    ui.label("Ring");
                    ui.add(egui::Slider::new(&mut p.pct, 0..=100).suffix("%"));
                    ui.end_row();
                });
            }
            Source::Command => {
                ui.label("Command");
                ui.add(
                    egui::TextEdit::multiline(&mut p.command)
                        .hint_text("any command, e.g. curl -s wttr.in/?format=%t")
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                ui.add(egui::Slider::new(&mut p.command_secs, 1..=3600).logarithmic(true).text("seconds between runs"));
                label_lo(ui, "First output line: value|ring%|subtitle|bottom line. Only value is required; a bare number 0-100 also fills the ring.");
                match &cmd_out {
                    Some(o) if o.error.is_some() => {
                        ui.colored_label(BAD, format!("Command failed: {}", o.error.clone().unwrap_or_default()));
                    }
                    Some(o) => {
                        ui.label(RichText::new(format!("Last output: {}", o.value)).monospace().color(OK));
                    }
                    None if !p.command.trim().is_empty() => label_lo(ui, "Running..."),
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
                    ui.colored_label(AMBER, "Not a valid date yet");
                }
                label_lo(ui, "The ring fills up as the date gets closer.");
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
                        if accent_button(ui, label).clicked() {
                            p.pomo_end = Some(now + p.pomo_left(now));
                            p.pomo_paused = None;
                        }
                    }
                    if ui.button("Reset").clicked() {
                        p.pomo_end = None;
                        p.pomo_paused = None;
                    }
                });
                label_lo(ui, "The ring drains as time runs out.");
            }
        }

        let after = serde_json::to_string(&self.s.customs[idx]).unwrap_or_default();
        if before != after && self.board_page == Some(page_id) {
            self.custom_dirty = true;
        }
        ui.add_space(8.0);
        let on = self.showing() == Slot::Page(page_id);
        if !on && accent_button(ui, "Show on screen").clicked() {
            self.show(Slot::Page(page_id));
        }
    }

    // ---- Claude / System / Clock ------------------------------------------

    fn claude_page(&mut self, ui: &mut Ui) {
        let (c, scanned) = {
            let sh = self.shared.lock().unwrap();
            (sh.claude.clone(), sh.claude_scanned)
        };
        let (block, today) = self.data_line(|d| (d.block_pct(), d.today_pct()));
        tiles(
            ui,
            vec![
                ("5-hour block", human(c.block_tokens), format!("{block}% of the ring"), Some(block as f32 / 100.0), CLAUDE),
                ("Resets in", c.time_left(), "current block".into(), None, CLAUDE),
                ("Today", human(c.today_tokens), format!("{today}% of your goal"), Some(today as f32 / 100.0), CLAUDE),
                ("Replies today", c.today_msgs.to_string(), format!("{} log files", c.log_files), None, CLAUDE),
            ],
        );
        if !scanned {
            label_lo(ui, "Scanning your logs...");
        }
        ui.add_space(6.0);
        card(ui, |ui| {
            card_title(ui, "5-hour ring", "What fills the ring on the Claude 5H widget");
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
        });
        ui.add_space(6.0);
        card(ui, |ui| {
            card_title(ui, "Daily goal", "Fills the ring on the Claude Today widget");
            ui.add(egui::Slider::new(&mut self.s.daily_goal_m, 0.5..=1000.0).logarithmic(true).suffix(" M tokens"));
        });
        ui.add_space(6.0);
        label_lo(ui, "Totals include cache reads. The 5-hour block is estimated from your logs, not Anthropic's official limit meter.");
    }

    fn system_page(&mut self, ui: &mut Ui) {
        let s = self.shared.lock().unwrap().sys.clone();
        let ram = s.ram_pct() as f32;
        tiles(
            ui,
            vec![
                ("CPU", format!("{:.0}%", s.cpu), "load".into(), Some(s.cpu / 100.0), level_color(s.cpu)),
                (
                    "GPU",
                    s.gpu.map(|g| format!("{g:.0}%")).unwrap_or("--".into()),
                    if s.gpu.is_some() { "load".into() } else { "no reading".into() },
                    s.gpu.map(|g| g / 100.0),
                    level_color(s.gpu.unwrap_or(0.0)),
                ),
                ("RAM", format!("{ram:.0}%"), format!("{:.1} / {:.1} GB", s.ram_used as f64 / 1e9, s.ram_total as f64 / 1e9), Some(ram / 100.0), level_color(ram)),
                ("Disk C:", format!("{}%", s.disk_pct), "used".into(), Some(s.disk_pct as f32 / 100.0), level_color(s.disk_pct as f32)),
            ],
        );
        ui.add_space(6.0);
        ui.columns(2, |cols| {
            card(&mut cols[0], |ui| {
                card_title(ui, "Network and uptime", "");
                egui::Grid::new("sys").num_columns(2).spacing([18.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new("Download").color(TEXT_MID));
                    ui.label(RichText::new(format!("{}B/s", human(s.net_rx))).strong());
                    ui.end_row();
                    ui.label(RichText::new("Upload").color(TEXT_MID));
                    ui.label(RichText::new(format!("{}B/s", human(s.net_tx))).strong());
                    ui.end_row();
                    ui.label(RichText::new("Uptime").color(TEXT_MID));
                    ui.label(RichText::new(format!("{}d {}h {}m", s.uptime / 86400, s.uptime / 3600 % 24, s.uptime / 60 % 60)).strong());
                    ui.end_row();
                });
            });
            card(&mut cols[1], |ui| {
                card_title(ui, "System widget", "Used by System and GT Gauge");
                ui.horizontal(|ui| {
                    ui.label("Title color");
                    ui.color_edit_button_srgb(&mut self.s.sys_color);
                });
                label_lo(ui, "GPU load comes from nvidia-smi, or Windows' GPU counters on other cards.");
            });
        });
    }

    fn clock_page(&mut self, ui: &mut Ui) {
        ui.columns(2, |cols| {
            card(&mut cols[0], |ui| {
                card_title(ui, "Look", "");
                toggle_row(ui, "24-hour time", "", &mut self.s.clock_24h);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Accent color");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.color_edit_button_srgb(&mut self.s.clock_color);
                    });
                });
            });
            card(&mut cols[1], |ui| {
                card_title(ui, "Bottom line", "Shown under the time");
                ui.radio_value(&mut self.s.clock_extra, ClockExtra::Utc, "UTC time");
                ui.radio_value(&mut self.s.clock_extra, ClockExtra::Week, "Week and day of year");
                ui.radio_value(&mut self.s.clock_extra, ClockExtra::Text, "My text");
                ui.radio_value(&mut self.s.clock_extra, ClockExtra::Nothing, "Nothing");
                if self.s.clock_extra == ClockExtra::Text {
                    ui.add(egui::TextEdit::singleline(&mut self.s.clock_text).char_limit(22).hint_text("up to 22 characters"));
                }
            });
        });
    }

    // ---- Settings ----------------------------------------------------------

    fn settings_page(&mut self, ui: &mut Ui) {
        ui.columns(2, |cols| {
            card(&mut cols[0], |ui| {
                card_title(ui, "Connection", "The Arduino's USB port");
                let connected = self.link.is_some();
                ui.add_enabled_ui(!connected, |ui| {
                    let selected = if self.s.port.is_empty() { "Pick a port".to_string() } else { self.s.port.clone() };
                    egui::ComboBox::from_id_salt("port").selected_text(selected).width(ui.available_width()).show_ui(ui, |ui| {
                        for p in &self.ports {
                            ui.selectable_value(&mut self.s.port, p.name.clone(), &p.label);
                        }
                        if self.ports.is_empty() {
                            ui.label("No serial ports");
                        }
                    });
                });
                toggle_row(ui, "Auto-connect", "Connect when the board is plugged in", &mut self.s.auto_connect);
                label_lo(ui, &self.status);
            });
            card(&mut cols[1], |ui| {
                card_title(ui, "App", &format!("OrbDash {VERSION}"));
                let mut autostart = self.s.start_with_windows;
                if toggle_row(ui, "Start with Windows", "Open OrbDash when you sign in", &mut autostart) {
                    match set_autostart(autostart) {
                        Ok(()) => {
                            self.s.start_with_windows = autostart;
                            self.app_error = None;
                        }
                        Err(e) => self.app_error = Some(e),
                    }
                }
                if let Some(e) = &self.app_error {
                    ui.colored_label(BAD, e);
                }
                label_lo(ui, "Settings are saved automatically.");
                ui.add_space(6.0);
                if ui.button(RichText::new("Reset all settings").color(BAD)).clicked() {
                    let port = std::mem::take(&mut self.s.port);
                    self.s = Settings { port, start_with_windows: self.s.start_with_windows, ..Settings::default() };
                    self.edit_page = self.s.customs.first().map(|p| p.id);
                    self.board_page = None;
                    self.preview = Slot::Mode(0);
                }
            });
        });
        ui.add_space(6.0);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                card_title(ui, "Serial log", "-> sent to the board   <- from the board");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("Clear").clicked() {
                        self.log.clear();
                    }
                });
            });
            Frame::new().fill(Color32::from_rgb(10, 11, 14)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(260.0).stick_to_bottom(true).auto_shrink([false, true]).show(ui, |ui| {
                    if self.log.is_empty() {
                        label_lo(ui, "Nothing yet.");
                    }
                    for (out, line) in &self.log {
                        let (arrow, col) = if *out { ("->", TEXT_LO) } else { ("<-", CYAN) };
                        ui.label(RichText::new(format!("{arrow} {line}")).monospace().size(12.0).color(col));
                    }
                });
            });
        });
    }

    // ---- Round preview -----------------------------------------------------

    /// What the round display shows, drawn `size` px across
    fn screen(&self, ui: &mut Ui, size: f32) {
        let slot = self.showing();
        let view = {
            let sh = self.shared.lock().unwrap();
            let (mode, page) = match slot {
                Slot::Mode(m) => (m, self.s.customs.first()),
                Slot::Page(id) => (MODE_CUSTOM, self.page(id)),
            };
            let cmd = page.and_then(|p| sh.cmds.get(&p.id));
            let d = Data { claude: &sh.claude, sys: &sh.sys, page, cmd, s: &self.s };
            d.view(mode)
        };

        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        let k = size / 240.0;
        let origin = rect.min;
        let at = |x: f32, y: f32| Pos2::new(origin.x + x * k, origin.y + y * k);
        let p = ui.painter();

        let center = at(120.0, 120.0);
        p.circle_filled(center, 126.0 * k, Color32::from_rgb(34, 36, 44));
        p.circle_filled(center, 120.0 * k, Color32::BLACK);
        p.circle_stroke(center, 118.0 * k, Stroke::new(1.0, TRACK));

        if let Some(g) = view.gauge {
            // No status line: on the board it would sit on the yellow ring
            draw_gauge(p, &at, k, g);
            return;
        }

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
            p.text(at(120.0, t.y), Align2::CENTER_TOP, &t.text, font, t.color);
        }
        let tag = match &self.board {
            Some(b) if b.ready => format!("LIVE [{}/{}]", b.mode + 1, MODE_NAMES.len()),
            _ => "PREVIEW".into(),
        };
        p.text(at(120.0, 200.0), Align2::CENTER_TOP, tag, FontId::monospace(8.0 * k), DIM);
    }
}

/// The OrbDash mark, `size` px across: same geometry as brand/make_brand.py
/// (a 270° gauge, three quarters full, violet to ember, head and orb)
fn draw_mark(p: &egui::Painter, c: Pos2, size: f32) {
    let s = size / 100.0;
    let at = |deg: f32, r: f32| {
        let a = deg.to_radians();
        Pos2::new(c.x + a.cos() * r * s, c.y + a.sin() * r * s)
    };
    let mix = |k: f32| {
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k) as u8;
        Color32::from_rgb(l(ACCENT.r(), EMBER.r()), l(ACCENT.g(), EMBER.g()), l(ACCENT.b(), EMBER.b()))
    };
    let track: Vec<Pos2> = (0..=54).map(|i| at(135.0 + i as f32 * 5.0, 36.0)).collect();
    p.add(egui::Shape::line(track, Stroke::new(11.0 * s, BORDER)));
    let end = 135.0 + 270.0 * 0.75;
    let n = 40;
    for i in 0..n {
        let (a0, a1) = (135.0 + (end - 135.0) * i as f32 / n as f32, 135.0 + (end - 135.0) * (i + 1) as f32 / n as f32);
        p.line_segment([at(a0, 36.0), at(a1, 36.0)], Stroke::new(11.0 * s, mix(i as f32 / n as f32)));
    }
    // Round caps
    p.circle_filled(at(135.0, 36.0), 5.5 * s, ACCENT);
    p.circle_filled(at(end, 36.0), 5.5 * s, EMBER);
    p.circle_filled(at(end, 36.0), 3.6 * s, TEXT_HI);
    p.circle_filled(c, 11.5 * s, ACCENT);
}

/// GT widget preview, same geometry as drawGtFace/gtNeedle in the firmware
fn draw_gauge(p: &egui::Painter, at: &dyn Fn(f32, f32) -> Pos2, k: f32, g: Gauge) {
    let pt = |pct: f32, r: f32| {
        let a = (135.0 + pct * 2.7f32).to_radians();
        at(120.0 + a.cos() * r, 120.0 + a.sin() * r)
    };
    let red = |pct: f32| pct >= GT_RED_FROM as f32;
    let yellow = Color32::from_rgb(230, 210, 0);
    let center = at(120.0, 120.0);

    // Yellow ring r80..103, red bar outside it over the redline
    p.circle_stroke(center, 91.5 * k, Stroke::new(23.0 * k, yellow));
    let bar: Vec<Pos2> = (0..=30).map(|i| pt(GT_RED_FROM as f32 + i as f32 * 0.5, 107.0)).collect();
    p.add(egui::Shape::line(bar, Stroke::new(4.0 * k, ALERT)));
    for t in (0..=100).step_by(2) {
        let t = t as f32;
        let (seg, w, col) = if t % 10.0 == 0.0 { ([105.0, 118.0], 3.0, TEXT) } else { ([111.0, 117.0], 1.0, yellow) };
        p.line_segment([pt(t, seg[0]), pt(t, seg[1])], Stroke::new(w * k, if red(t) { ALERT } else { col }));
    }
    for i in 0..=10 {
        // Bold, like the firmware: drawn twice, 1 px apart
        for dx in [0.0, 1.0] {
            let font = FontId::monospace(17.0 * k);
            p.text(pt(i as f32 * 10.0, 91.5) + vec2(dx * k, 0.0), Align2::CENTER_CENTER, i.to_string(), font, Color32::BLACK);
        }
    }

    // Needle and silver hub
    let cpu = g.cpu.min(100) as f32;
    p.line_segment([pt(cpu, 10.0), pt(cpu, 70.0)], Stroke::new(3.0 * k, TEXT));
    p.line_segment([pt(cpu, 70.0), pt(cpu, 100.0)], Stroke::new(1.2 * k, TEXT));
    p.circle_filled(center, 8.0 * k, Color32::from_gray(128));
    p.circle_filled(center, 5.0 * k, Color32::from_gray(195));

    // Readout under the hub
    let font = FontId::monospace(8.0 * k);
    let hot = if red(cpu) { ALERT } else { TEXT };
    p.text(at(120.0, 146.0), Align2::CENTER_TOP, g.cpu.to_string(), FontId::monospace(24.0 * k), hot);
    p.text(at(120.0, 170.0), Align2::CENTER_TOP, "CPU %", font.clone(), DIM);
    let val = |v: Option<u8>| v.map(|v| format!("{v:>3}")).unwrap_or(" --".into());
    let mut job = egui::text::LayoutJob::default();
    for (label, v) in [("GPU", g.gpu), ("  RAM", Some(g.ram))] {
        let col = if v.is_some_and(|v| red(v as f32)) { ALERT } else { TEXT };
        job.append(label, 0.0, egui::TextFormat::simple(font.clone(), DIM));
        job.append(&val(v), 0.0, egui::TextFormat::simple(font.clone(), col));
    }
    let galley = p.layout_job(job);
    p.galley(at(120.0, 180.0) - Vec2::new(galley.size().x / 2.0, 0.0), galley, TEXT);
    p.text(at(120.0, 226.0), Align2::CENTER_TOP, "x10 %", font, DIM);
}

/// "+ New" dropdown of page templates; returns the picked one
fn template_menu(ui: &mut Ui, label: &str) -> Option<CustomPage> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(("templates", label)).selected_text(label).width(96.0).show_ui(ui, |ui| {
        ui.set_min_width(250.0);
        for (name, page) in custom::templates() {
            if ui.selectable_label(false, name).clicked() {
                picked = Some(page);
            }
        }
    });
    picked
}
