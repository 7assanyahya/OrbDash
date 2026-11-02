"""Wiring diagrams for OrbDash Mini (ESP32-C3) and OrbDash Pro (ESP32-S3).
Writes docs/wiring_mini.svg/png and docs/wiring_pro.svg/png (PNGs via
headless Microsoft Edge). Run: python docs/wiring_esp.py
Same style as docs/wiring.py; pins match firmware/OrbDashESP.
"""
import pathlib
import subprocess
import time

HERE = pathlib.Path(__file__).resolve().parent
EDGE = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"

BG, CARD, INK, MUTED = "#f6f7f9", "#ffffff", "#1f2933", "#5f6b7a"
MCU, MCU_EDGE = "#1f2328", "#0d0f12"
PCB, PCB_EDGE = "#1d2b4f", "#121b33"
WIRE = {"VCC": "#e63946", "GND": "#222222", "SCL": "#c026d3", "SDA": "#f77f00",
        "DC": "#2a9d4b", "CS": "#d4a017", "RST": "#0ea5e9"}
DISPLAY_PINS = ["VCC", "GND", "SCL", "SDA", "DC", "CS", "RST"]


class Svg:
    def __init__(self, w, h):
        self.w, self.h, self.out = w, h, []
        self.add(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" '
                 f'font-family="Segoe UI, Helvetica, Arial, sans-serif">')
        self.add(f'<rect width="{w}" height="{h}" rx="18" fill="{BG}"/>')

    def add(self, s):
        self.out.append(s)

    def text(self, x, y, s, size=13, fill=INK, anchor="start", weight="normal"):
        self.add(f'<text x="{x}" y="{y}" font-size="{size}" fill="{fill}" text-anchor="{anchor}" font-weight="{weight}">{s}</text>')

    def wire(self, pts, color, w=4):
        d = "M" + " L".join(f"{x},{y}" for x, y in pts)
        self.add(f'<path d="{d}" fill="none" stroke="#ffffff" stroke-width="{w + 3}" stroke-linejoin="round" stroke-linecap="round"/>')
        self.add(f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{w}" stroke-linejoin="round" stroke-linecap="round"/>')

    def dot(self, x, y, color, r=5):
        self.add(f'<circle cx="{x}" cy="{y}" r="{r}" fill="{color}" stroke="#fff" stroke-width="1.5"/>')

    def save(self, name):
        self.add("</svg>")
        svg = HERE / f"{name}.svg"
        svg.write_text("\n".join(self.out), encoding="utf-8")
        png = HERE / f"{name}.png"
        png.unlink(missing_ok=True)
        html = HERE / f".{name}.html"
        html.write_text(f'<html><body style="margin:0">{svg.read_text(encoding="utf-8")}</body></html>', encoding="utf-8")
        subprocess.run([EDGE, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                        f"--user-data-dir={HERE / '.edge'}", f"--window-size={self.w},{self.h}",
                        f"--screenshot={png}", html.as_uri()], check=True, capture_output=True)
        for _ in range(100):
            if png.exists() and png.stat().st_size:
                break
            time.sleep(0.2)
        html.unlink()
        print("wrote", svg.name, png.name)


def board(s, x, y, w, h, title, sub, pins, pitch):
    """MCU board with labelled pins down its right edge. Returns {pin: (x, y)}."""
    s.add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="12" fill="{MCU}" stroke="{MCU_EDGE}" stroke-width="3"/>')
    s.add(f'<rect x="{x + w / 2 - 22}" y="{y - 14}" width="44" height="22" rx="5" fill="#b8c0c8" stroke="#7d8790" stroke-width="2"/>')
    s.text(x + w / 2, y + 4, "USB-C", 10, "#3b4650", "middle", "700")
    s.text(x + 24, y + 48, title, 18, "#ffffff", weight="700")
    s.text(x + 24, y + 70, sub, 12, "#9aa4b2")
    at = {}
    for i, (pin, label) in enumerate(pins):
        py = y + 110 + i * pitch
        s.add(f'<rect x="{x + w - 16}" y="{py - 7}" width="14" height="14" rx="2" fill="#d7dde3"/>')
        s.text(x + w - 26, py + 5, pin, 13, "#ffffff", "end", "700")
        if label:
            s.text(x + w - 26, py + 21, label, 10.5, "#9aa4b2", "end")
        at[pin] = (x + w - 2, py)
    return at


def display(s, x, y, name, pitch=34):
    """Round GC9A01 module: header tab on its left, the screen on its right."""
    tab_h = pitch * 6 + 40
    s.add(f'<rect x="{x}" y="{y}" width="92" height="{tab_h}" rx="10" fill="{PCB}" stroke="{PCB_EDGE}" stroke-width="3"/>')
    cy = y + tab_h / 2
    s.add(f'<circle cx="{x + 200}" cy="{cy}" r="{tab_h / 2 + 18}" fill="{PCB}" stroke="{PCB_EDGE}" stroke-width="3"/>')
    s.add(f'<circle cx="{x + 200}" cy="{cy}" r="{tab_h / 2 - 6}" fill="#000"/>')
    s.add(f'<path d="M{x + 200 - tab_h * 0.32},{cy + tab_h * 0.18} A{tab_h * 0.37},{tab_h * 0.37} 0 1 1 {x + 200 + tab_h * 0.32},{cy + tab_h * 0.18}" '
          f'fill="none" stroke="#8B5CF6" stroke-width="9" stroke-linecap="round"/>')
    s.text(x + 200, cy + 6, name, 15, "#ffffff", "middle", "700")
    at = {}
    for i, pin in enumerate(DISPLAY_PINS):
        py = y + 20 + i * pitch
        s.add(f'<circle cx="{x + 14}" cy="{py}" r="6" fill="#d7dde3" stroke="#8a95a3"/>')
        s.text(x + 28, py + 5, pin, 13, "#ffffff", weight="700")
        at[pin] = (x + 8, py)
    return at


def legend(s, x, y, rows):
    s.add(f'<rect x="{x}" y="{y}" width="{s.w - 2 * x}" height="{40 + 22 * len(rows)}" rx="12" fill="{CARD}" stroke="#e2e6ea"/>')
    cols = [x + 24, x + 150, x + 290, x + 520]
    for c, head in zip(cols, ["Display", "Wire", "Board pin", "Notes"]):
        s.text(c, y + 28, head, 13, INK, weight="700")
    for i, (pin, board_pin, note) in enumerate(rows):
        ry = y + 52 + i * 22
        s.text(cols[0], ry, pin, 12.5, INK, weight="600")
        s.add(f'<rect x="{cols[1]}" y="{ry - 10}" width="26" height="10" rx="3" fill="{WIRE[pin.split()[0]]}"/>')
        s.text(cols[2], ry, board_pin, 12.5, INK)
        s.text(cols[3], ry, note, 12.5, MUTED)


# ---------------------------------------------------------------------------
# Mini: ESP32-C3 SuperMini + one display. Pins in display order: straight wires.
# ---------------------------------------------------------------------------
s = Svg(1100, 830)
s.text(40, 48, "OrbDash Mini wiring", 28, weight="700")
s.text(40, 74, "ESP32-C3 SuperMini + one 1.28&#8221; round GC9A01 display. Everything runs at 3.3 V.", 14, MUTED)
mini_pins = [("3V3", ""), ("GND", ""), ("GPIO4", "SPI SCK"), ("GPIO6", "SPI MOSI"), ("GPIO10", ""), ("GPIO7", ""), ("GPIO3", "")]
b = board(s, 70, 130, 280, 410, "ESP32-C3", "SuperMini", mini_pins, 46)
d = display(s, 700, 140, "MINI", pitch=46)
for i, ((pin, _), dp) in enumerate(zip(mini_pins, DISPLAY_PINS)):
    (x0, y0), (x1, y1) = b[pin], d[dp]
    mx = 440 + i * 30  # each wire turns at its own x, so none overlap
    s.wire([(x0, y0), (mx, y0), (mx, y1), (x1, y1)], WIRE[dp])
s.text(90, 578, "Button: the board's BOOT button (GPIO9). LED: the on-board blue LED (GPIO8).", 13, MUTED)
s.text(90, 598, "An extra 7 mm button on top of the case goes between GPIO9 and GND.", 13, MUTED)
legend(s, 40, 620, [("VCC", "3V3", "not 5V: the board's 3.3 V pin"), ("GND", "GND", ""),
                    ("SCL", "GPIO4", "SPI clock"), ("SDA", "GPIO6", "SPI data (MOSI)"),
                    ("DC", "GPIO10", ""), ("CS", "GPIO7", ""), ("RST", "GPIO3", "")])
s.save("wiring_mini")

# ---------------------------------------------------------------------------
# Pro: ESP32-S3 DevKit + three displays. Shared lines run down a bus and
# branch to every display; each display has its own CS.
# ---------------------------------------------------------------------------
s = Svg(1200, 1360)
s.text(40, 48, "OrbDash Pro wiring", 28, weight="700")
s.text(40, 74, "ESP32-S3 DevKit + three 1.28&#8221; GC9A01 displays. VCC, GND, SCL, SDA, DC and RST are shared; "
               "each display has its own CS.", 14, MUTED)
pro_pins = [("3V3", ""), ("GND", ""), ("GPIO12", "SPI SCK"), ("GPIO11", "SPI MOSI"), ("GPIO9", ""),
            ("GPIO8", ""), ("GPIO10", "CS left"), ("GPIO13", "CS middle"), ("GPIO14", "CS right")]
b = board(s, 60, 300, 260, 560, "ESP32-S3", "DevKit", pro_pins, 46)
shared = [("3V3", "VCC"), ("GND", "GND"), ("GPIO12", "SCL"), ("GPIO11", "SDA"), ("GPIO9", "DC"), ("GPIO8", "RST")]
cs_pins = ["GPIO10", "GPIO13", "GPIO14"]
names = ["LEFT", "MIDDLE", "RIGHT"]
disp = [display(s, 860, 110 + k * 330, names[k], pitch=30) for k in range(3)]

bus_x0 = 420
for k, (bp, dp) in enumerate(shared):
    bx = bus_x0 + k * 30
    x0, y0 = b[bp]
    ys = [dd[dp][1] for dd in disp]
    s.wire([(x0, y0), (bx, y0)], WIRE[dp])
    s.wire([(bx, min(ys + [y0])), (bx, max(ys + [y0]))], WIRE[dp])
    for dd in disp:
        x1, y1 = dd[dp]
        s.wire([(bx, y1), (x1, y1)], WIRE[dp], 3)
        s.dot(bx, y1, WIRE[dp], 4)
    s.dot(bx, y0, WIRE[dp], 4)
for k, bp in enumerate(cs_pins):
    bx = bus_x0 + (len(shared) + k) * 30
    (x0, y0), (x1, y1) = b[bp], disp[k]["CS"]
    s.wire([(x0, y0), (bx, y0), (bx, y1), (x1, y1)], WIRE["CS"])
    s.text(bx + 6, (y0 + y1) / 2, f"CS{k + 1}", 11, MUTED)
s.text(80, 1040, "Tip: join the shared lines on a small strip of perfboard (or Wago-style connectors),", 13, MUTED)
s.text(80, 1060, "then run one wire from there to each display. Button: BOOT (GPIO0). LED: on-board RGB (GPIO48).", 13, MUTED)
legend(s, 40, 1080, [("VCC", "3V3", "all three displays (not 5V)"), ("GND", "GND", "all three"),
                    ("SCL", "GPIO12", "SPI clock, all three"), ("SDA", "GPIO11", "SPI data (MOSI), all three"),
                    ("DC", "GPIO9", "all three"), ("RST", "GPIO8", "all three"),
                    ("CS left", "GPIO10", "left display only"), ("CS middle", "GPIO13", "middle display only"),
                    ("CS right", "GPIO14", "right display only")])
s.save("wiring_pro")
