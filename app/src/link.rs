//! Serial link to the board. A worker thread owns the port: it writes queued
//! lines (paced, so the Uno's small RX buffer never overflows while it is
//! busy drawing) and reports every line the board sends back.
//! Commands (W/R/A) jump the queue; a newer data line for a widget replaces
//! an older one still waiting, so the queue never fills with stale data.

use eframe::egui;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

pub const BAUD: u32 = 115_200;

/// Gap between outgoing lines. A full-screen redraw on the Uno takes ~100 ms,
/// so lines are trickled out rather than sent in one burst.
const LINE_GAP: Duration = Duration::from_millis(90);

pub enum LinkEvent {
    Line(String),
    Closed(Option<String>),
}

/// Each connection has its own event channel, so a stale "closed" from an
/// old connection can never tear down a newer one.
pub struct Link {
    tx: Sender<String>,
    events: Receiver<LinkEvent>,
    pub port: String,
}

impl Link {
    pub fn open(port: &str, ctx: egui::Context) -> Result<Link, String> {
        let mut serial = serialport::new(port, BAUD)
            .timeout(Duration::from_millis(15))
            .open()
            .map_err(|e| e.to_string())?;

        let (tx, rx) = mpsc::channel::<String>();
        let (events, events_rx) = mpsc::channel();
        thread::spawn(move || {
            let reason = run(&mut *serial, rx, &events, &ctx);
            let _ = events.send(LinkEvent::Closed(reason));
            ctx.request_repaint();
        });

        Ok(Link { tx, events: events_rx, port: port.to_string() })
    }

    pub fn poll(&self) -> Vec<LinkEvent> {
        self.events.try_iter().collect()
    }

    pub fn send(&self, line: String) {
        let _ = self.tx.send(line);
    }
}

fn run(
    serial: &mut dyn serialport::SerialPort,
    rx: Receiver<String>,
    events: &Sender<LinkEvent>,
    ctx: &egui::Context,
) -> Option<String> {
    let mut buf = [0u8; 256];
    let mut pending = Vec::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut last_write = Instant::now() - LINE_GAP;

    loop {
        loop {
            match rx.try_recv() {
                Ok(line) => enqueue(&mut queue, line),
                // The app dropped the Link: close quietly
                Err(TryRecvError::Disconnected) => return None,
                Err(TryRecvError::Empty) => break,
            }
        }

        if last_write.elapsed() >= LINE_GAP {
            if let Some(line) = queue.pop_front() {
                let mut bytes = line.into_bytes();
                bytes.push(b'\n');
                if let Err(e) = serial.write_all(&bytes).and_then(|_| serial.flush()) {
                    return Some(e.to_string());
                }
                last_write = Instant::now();
            }
        }

        match serial.read(&mut buf) {
            Ok(n) => {
                for &b in &buf[..n] {
                    match b {
                        b'\n' => {
                            let line = String::from_utf8_lossy(&pending).trim().to_string();
                            pending.clear();
                            if !line.is_empty() {
                                let _ = events.send(LinkEvent::Line(line));
                                ctx.request_repaint();
                            }
                        }
                        b'\r' => {}
                        _ if pending.len() < 200 => pending.push(b),
                        _ => pending.clear(),
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Some(e.to_string()),
        }
    }
}

fn enqueue(queue: &mut VecDeque<String>, line: String) {
    let kind = line.split('|').next().unwrap_or("").to_string();
    match kind.as_str() {
        "W" | "R" | "A" => {
            // Behind any other waiting commands, ahead of all data
            let is_cmd = |l: &String| matches!(l.split('|').next(), Some("W" | "R" | "A"));
            let at = queue.iter().take_while(|l| is_cmd(l)).count();
            queue.insert(at, line);
        }
        _ => match queue.iter_mut().find(|l| l.split('|').next() == Some(kind.as_str())) {
            Some(old) => *old = line,
            None => queue.push_back(line),
        },
    }
}

pub struct PortInfo {
    pub name: String,
    pub label: String,
    pub looks_like_arduino: bool,
}

/// USB vendors of the boards OrbDash runs on: Arduino (2), WCH CH340/CH343,
/// FTDI, Silicon Labs CP210x, Espressif (ESP32-C3/S3 native USB).
/// Matched by ID, not by name: "USB Serial Device" is also what a 3D printer
/// shows up as, and auto-connect must never pick one of those.
const BOARD_VIDS: [u16; 6] = [0x2341, 0x2A03, 0x1A86, 0x0403, 0x10C4, 0x303A];

pub fn list_ports() -> Vec<PortInfo> {
    let mut ports: Vec<PortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let (desc, vid) = match &p.port_type {
                serialport::SerialPortType::UsbPort(u) => {
                    let s = format!(
                        "{} {}",
                        u.manufacturer.clone().unwrap_or_default(),
                        u.product.clone().unwrap_or_default()
                    );
                    (s.trim().to_string(), Some(u.vid))
                }
                _ => (String::new(), None),
            };
            let looks_like_arduino = vid.is_some_and(|v| BOARD_VIDS.contains(&v));
            let label = if desc.is_empty() {
                p.port_name.clone()
            } else {
                format!("{} - {}", p.port_name, desc)
            };
            PortInfo { name: p.port_name, label, looks_like_arduino }
        })
        .collect();
    ports.sort_by_key(|p| !p.looks_like_arduino);
    ports
}
