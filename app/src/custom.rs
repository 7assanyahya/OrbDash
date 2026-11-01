//! Custom pages: any number of user widgets that share the board's CUSTOM
//! slot. Each page gets its content from a source: fixed text, a command,
//! a countdown to a date, or a pomodoro timer.

use chrono::{Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Source {
    Static,
    Command,
    Countdown,
    Pomodoro,
}

impl Source {
    pub const ALL: [Source; 4] = [Source::Static, Source::Command, Source::Countdown, Source::Pomodoro];

    pub fn label(self) -> &'static str {
        match self {
            Source::Static => "Fixed text",
            Source::Command => "Command output",
            Source::Countdown => "Countdown to a date",
            Source::Pomodoro => "Pomodoro timer",
        }
    }

    /// Sources whose value changes every second
    pub fn ticks(self) -> bool {
        matches!(self, Source::Countdown | Source::Pomodoro)
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct CustomPage {
    pub id: u64,
    pub in_rotation: bool,
    pub title: String,
    pub value: String,
    pub subtitle: String,
    pub line: String,
    pub pct: u8,
    pub color: [u8; 3],
    pub source: Source,

    pub command: String,
    pub command_secs: u32,

    /// "YYYY-MM-DD HH:MM", local time
    pub countdown_to: String,
    /// Unix time the countdown was set, so the ring can show progress
    pub countdown_from: i64,

    pub pomo_minutes: u32,
    /// Unix time the running timer ends
    pub pomo_end: Option<i64>,
    /// Seconds left while paused
    pub pomo_paused: Option<i64>,
}

impl Default for CustomPage {
    fn default() -> Self {
        CustomPage {
            id: 0,
            in_rotation: true,
            title: "NOTE".into(),
            value: "HI".into(),
            subtitle: "MY WIDGET".into(),
            line: "edit me in the app".into(),
            pct: 50,
            color: [156, 105, 255],
            source: Source::Static,
            command: String::new(),
            command_secs: 10,
            countdown_to: String::new(),
            countdown_from: 0,
            pomo_minutes: 25,
            pomo_end: None,
            pomo_paused: None,
        }
    }
}

/// Ready-made pages for the "Add from template" menu
pub fn templates() -> Vec<(&'static str, CustomPage)> {
    let base = CustomPage::default();
    let in_30_days = (Local::now() + chrono::Duration::days(30)).format("%Y-%m-%d 00:00").to_string();
    vec![
        ("Note", base.clone()),
        (
            "Weather (wttr.in)",
            CustomPage {
                title: "WEATHER".into(),
                value: "--".into(),
                subtitle: "OUTSIDE".into(),
                line: "humidity on the ring".into(),
                color: [80, 170, 255],
                source: Source::Command,
                // value | ring % (humidity) | subtitle
                command: "curl -s \"wttr.in/?format=%t|%h|%C\"".into(),
                command_secs: 900,
                ..base.clone()
            },
        ),
        (
            "Laptop battery",
            CustomPage {
                title: "BATTERY".into(),
                value: "--".into(),
                subtitle: "CHARGE".into(),
                line: String::new(),
                color: [0, 220, 120],
                source: Source::Command,
                command: "powershell -NoProfile -Command \"(Get-CimInstance Win32_Battery).EstimatedChargeRemaining\"".into(),
                command_secs: 60,
                ..base.clone()
            },
        ),
        (
            "Countdown",
            CustomPage {
                title: "COUNTDOWN".into(),
                subtitle: "TIME LEFT".into(),
                line: String::new(),
                color: [255, 165, 0],
                source: Source::Countdown,
                countdown_to: in_30_days,
                countdown_from: Local::now().timestamp(),
                ..base.clone()
            },
        ),
        (
            "Pomodoro",
            CustomPage {
                title: "FOCUS".into(),
                subtitle: "POMODORO".into(),
                line: "start it from the app".into(),
                color: [255, 90, 90],
                source: Source::Pomodoro,
                ..base
            },
        ),
    ]
}

/// Output of a page's command: `value|ring%|subtitle|bottom line`
#[derive(Clone, Default)]
pub struct CmdOutput {
    pub value: String,
    pub pct: Option<u8>,
    pub subtitle: Option<String>,
    pub line: Option<String>,
    pub error: Option<String>,
}

impl CmdOutput {
    pub fn parse(out: &str) -> CmdOutput {
        let first = out.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        let mut f = first.split('|').map(|s| s.trim().to_string());
        let value = f.next().unwrap_or_default();
        let num = |s: &str| s.trim().trim_end_matches('%').parse::<f32>().ok();
        let pct = f.next().and_then(|p| num(&p));
        // A bare number 0-100 also drives the ring
        let pct = pct.or_else(|| num(&value).filter(|v| (0.0..=100.0).contains(v)));
        CmdOutput {
            value,
            pct: pct.map(|p| p.clamp(0.0, 100.0) as u8),
            subtitle: f.next().filter(|s| !s.is_empty()),
            line: f.next().filter(|s| !s.is_empty()),
            error: None,
        }
    }
}

/// What a page shows right now
pub struct Resolved {
    pub title: String,
    pub value: String,
    pub subtitle: String,
    pub line: String,
    pub pct: u8,
    pub color: [u8; 3],
}

pub fn parse_when(s: &str) -> Option<i64> {
    let naive = NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M").ok()?;
    Local.from_local_datetime(&naive).earliest().map(|d| d.timestamp())
}

fn fmt_left(secs: i64) -> String {
    let s = secs.max(0);
    if s >= 86400 {
        format!("{}d{:02}h", s / 86400, s / 3600 % 24)
    } else if s >= 3600 {
        format!("{}h{:02}m", s / 3600, s / 60 % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

impl CustomPage {
    pub fn pomo_left(&self, now: i64) -> i64 {
        match (self.pomo_end, self.pomo_paused) {
            (Some(end), _) => end - now,
            (None, Some(left)) => left,
            _ => self.pomo_minutes as i64 * 60,
        }
    }

    pub fn resolve(&self, cmd: Option<&CmdOutput>) -> Resolved {
        let now = Local::now().timestamp();
        let mut r = Resolved {
            title: self.title.clone(),
            value: self.value.clone(),
            subtitle: self.subtitle.clone(),
            line: self.line.clone(),
            pct: self.pct,
            color: self.color,
        };
        match self.source {
            Source::Static => {}
            Source::Command => match cmd {
                Some(o) if o.error.is_some() => {
                    r.value = "ERR".into();
                    r.line = o.error.clone().unwrap_or_default();
                }
                Some(o) => {
                    r.value = o.value.clone();
                    if let Some(p) = o.pct { r.pct = p; }
                    if let Some(s) = &o.subtitle { r.subtitle = s.clone(); }
                    if let Some(l) = &o.line { r.line = l.clone(); }
                }
                None => r.value = "...".into(),
            },
            Source::Countdown => match parse_when(&self.countdown_to) {
                Some(target) => {
                    let left = target - now;
                    r.value = if left <= 0 { "DONE".into() } else { fmt_left(left) };
                    let total = (target - self.countdown_from).max(1);
                    r.pct = (((total - left.max(0)) * 100) / total).clamp(0, 100) as u8;
                    if r.line.is_empty() {
                        r.line = format!("UNTIL {}", self.countdown_to.trim());
                    }
                }
                None => {
                    r.value = "SET DATE".into();
                    r.pct = 0;
                }
            },
            Source::Pomodoro => {
                let total = (self.pomo_minutes.max(1) as i64) * 60;
                let left = self.pomo_left(now);
                let running = self.pomo_end.is_some();
                if running && left <= 0 {
                    r.value = "DONE".into();
                    r.pct = 0;
                    r.line = "TIME FOR A BREAK".into();
                } else {
                    r.value = fmt_left(left);
                    r.pct = ((left.max(0) * 100) / total).clamp(0, 100) as u8;
                    if !running {
                        r.line = if self.pomo_paused.is_some() { "PAUSED".into() } else { "READY".into() };
                    }
                }
            }
        }
        r
    }
}
