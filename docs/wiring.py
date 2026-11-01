"""Generate docs/wiring.svg for OrbDash. Run: python docs/wiring.py"""
import sys

W, H = 1200, 1010
out = []
add = out.append

# ---- palette ---------------------------------------------------------------
BG, CARD, INK, MUTED = "#f6f7f9", "#ffffff", "#1f2933", "#5f6b7a"
BOARD, BOARD_EDGE = "#0f7c84", "#0a5b61"
PCB, PCB_EDGE = "#1d2b4f", "#121b33"
WIRE = {
    "VCC": ("#e63946", "Red"),
    "GND": ("#222222", "Black"),
    "SCL": ("#c026d3", "Magenta"),
    "SDA": ("#f77f00", "Orange"),
    "DC": ("#2a9d4b", "Green"),
    "CS": ("#d4a017", "Yellow"),
    "RST": ("#0ea5e9", "Cyan"),
}
BTN_WIRE, LED_WIRE = "#64748b", "#8b5cf6"

def text(x, y, s, size=13, fill=INK, anchor="start", weight="normal", extra=""):
    add(f'<text x="{x}" y="{y}" font-size="{size}" fill="{fill}" text-anchor="{anchor}" '
        f'font-weight="{weight}" {extra}>{s}</text>')

def wire(d, color, w=4):
    add(f'<path d="{d}" fill="none" stroke="#ffffff" stroke-width="{w + 3}" stroke-linejoin="round" stroke-linecap="round"/>')
    add(f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{w}" stroke-linejoin="round" stroke-linecap="round"/>')

def dot(x, y, color):
    add(f'<circle cx="{x}" cy="{y}" r="5" fill="{color}" stroke="#fff" stroke-width="1.5"/>')

add(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" '
    f'font-family="Segoe UI, Helvetica, Arial, sans-serif">')
add(f'<rect width="{W}" height="{H}" rx="18" fill="{BG}"/>')

# ---- title -----------------------------------------------------------------
text(40, 48, "OrbDash wiring", 28, INK, weight="700")
text(40, 74, "Arduino Uno + 1.28&#8221; round GC9A01 display, plus optional button and LED", 14, MUTED)

# ---- Arduino Uno -----------------------------------------------------------
BX, BY, BW, BH = 60, 340, 560, 320
add(f'<rect x="{BX}" y="{BY}" width="{BW}" height="{BH}" rx="14" fill="{BOARD}" stroke="{BOARD_EDGE}" stroke-width="3"/>')
# USB-B and barrel jack on the left edge
add(f'<rect x="36" y="380" width="74" height="62" rx="4" fill="#b8c0c8" stroke="#7d8790" stroke-width="2"/>')
text(73, 416, "USB", 13, "#3b4650", "middle", "700")
add(f'<rect x="40" y="560" width="58" height="52" rx="4" fill="#2b2b2b"/>')
text(69, 591, "DC", 11, "#aaa", "middle")
# MCU
add(f'<rect x="300" y="470" width="220" height="56" rx="4" fill="#1f2328"/>')
text(410, 503, "ATmega328P", 13, "#cfd6dd", "middle")
text(340, 450, "ARDUINO UNO", 22, "#ffffff", "middle", "700", 'letter-spacing="2"')

TOP_Y, BOT_Y = 362, 638
top = {}
for i, name in enumerate(["AREF", "GND", "13", "12", "11", "10", "9", "8"]):
    top[name] = 150 + i * 26
for i, name in enumerate(["7", "6", "5", "4", "3", "2", "1", "0"]):
    top[name] = 380 + i * 26
bot = {}
for i, name in enumerate(["IOREF", "RESET", "3.3V", "5V", "GND", "GND2", "VIN"]):
    bot[name] = 230 + i * 26
for i in range(6):
    bot[f"A{i}"] = 440 + i * 26

used_top = {"GND", "13", "11", "10", "9", "8", "3", "2"}
used_bot = {"5V", "GND2"}

def header(xs, y, used, label_dy, labels):
    xs_sorted = sorted(xs.values())
    add(f'<rect x="{xs_sorted[0] - 11}" y="{y - 11}" width="{xs_sorted[-1] - xs_sorted[0] + 22}" height="22" fill="#111" rx="2"/>')
    for name, x in xs.items():
        on = name in used
        add(f'<rect x="{x - 7}" y="{y - 7}" width="14" height="14" fill="{"#f2c94c" if on else "#444"}"/>')
        add(f'<circle cx="{x}" cy="{y}" r="3" fill="#111"/>')
        lab = labels.get(name, name)
        weight = "700" if on else "normal"
        col = "#ffffff" if on else "#bfe6e8"
        add(f'<text x="{x}" y="{y + label_dy}" font-size="11" fill="{col}" font-weight="{weight}" '
            f'text-anchor="start" transform="rotate(-90 {x} {y + label_dy})" dx="0" dy="4">{lab}</text>'
            if label_dy < 0 else
            f'<text x="{x}" y="{y + label_dy}" font-size="11" fill="{col}" font-weight="{weight}" '
            f'text-anchor="start" transform="rotate(90 {x} {y + label_dy})" dy="4">{lab}</text>')

header({k: top[k] for k in ["AREF", "GND", "13", "12", "11", "10", "9", "8"]}, TOP_Y, used_top, 18,
       {"11": "~11", "10": "~10", "9": "~9"})
header({k: top[k] for k in ["7", "6", "5", "4", "3", "2", "1", "0"]}, TOP_Y, used_top, 18,
       {"6": "~6", "5": "~5", "3": "~3", "1": "TX 1", "0": "RX 0"})
header({k: bot[k] for k in ["IOREF", "RESET", "3.3V", "5V", "GND", "GND2", "VIN"]}, BOT_Y, used_bot, -18,
       {"GND2": "GND"})
header({k: bot[k] for k in [f"A{i}" for i in range(6)]}, BOT_Y, set(), -18, {})
text(300, 418, "DIGITAL", 11, "#bfe6e8", "middle", extra='letter-spacing="1"')
text(300, 578, "POWER", 11, "#bfe6e8", "middle", extra='letter-spacing="1"')
text(505, 578, "ANALOG IN", 11, "#bfe6e8", "middle", extra='letter-spacing="1"')

# ---- Display module (drawn with its pin header on the left) ----------------
MX, MY, MW, MH = 820, 20, 300, 270
add(f'<rect x="{MX}" y="{MY}" width="{MW}" height="{MH}" rx="16" fill="{PCB}" stroke="{PCB_EDGE}" stroke-width="3"/>')
cx, cy, r = 1000, 155, 108
add(f'<circle cx="{cx}" cy="{cy}" r="{r + 6}" fill="#0b0f1a"/>')
add(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="#000"/>')
# mini OrbDash face: 270 deg ring, 60 % filled
import math
def arc_pts(a0, a1, rr):
    p0 = (cx + rr * math.cos(math.radians(a0)), cy + rr * math.sin(math.radians(a0)))
    p1 = (cx + rr * math.cos(math.radians(a1)), cy + rr * math.sin(math.radians(a1)))
    large = 1 if (a1 - a0) % 360 > 180 else 0
    return f"M{p0[0]:.1f},{p0[1]:.1f} A{rr},{rr} 0 {large} 1 {p1[0]:.1f},{p1[1]:.1f}"
add(f'<path d="{arc_pts(135, 405, 92)}" fill="none" stroke="#1b1f1b" stroke-width="12"/>')
add(f'<path d="{arc_pts(135, 135 + 162, 92)}" fill="none" stroke="#d97757" stroke-width="12"/>')
text(cx, cy - 34, "CLAUDE", 15, "#d97757", "middle", "700")
text(cx, cy + 12, "60%", 34, "#fff", "middle", "700")
text(cx, cy + 36, "1.28&#8221; GC9A01", 11, "#7b7d7b", "middle")
text(cx, MY + MH - 10, "240 x 240 round TFT", 11, "#9fb0d0", "middle")

pins = ["VCC", "GND", "SCL", "SDA", "DC", "CS", "RST"]
PX = 838
py = {p: 60 + i * 30 for i, p in enumerate(pins)}
add(f'<rect x="{PX - 10}" y="{py["VCC"] - 12}" width="20" height="{py["RST"] - py["VCC"] + 24}" rx="3" fill="#111"/>')
for p in pins:
    add(f'<circle cx="{PX}" cy="{py[p]}" r="7" fill="#f2c94c"/>')
    add(f'<circle cx="{PX}" cy="{py[p]}" r="3" fill="#111"/>')
    text(PX + 16, py[p] + 4, p, 12, "#ffffff", weight="700")

# ---- Display wires ---------------------------------------------------------
dest = {"GND": top["GND"], "SCL": top["13"], "SDA": top["11"], "DC": top["9"], "CS": top["10"], "RST": top["8"]}
for p in ["GND", "SCL", "SDA", "DC", "RST"]:
    wire(f"M{PX},{py[p]} H{dest[p]} V{TOP_Y}", WIRE[p][0])
# CS hops over the DC wire
hx = dest["DC"]
wire(f"M{PX},{py['CS']} H{hx + 9} A9,9 0 0 0 {hx - 9},{py['CS']} H{dest['CS']} V{TOP_Y}", WIRE["CS"][0])
# VCC runs over the top and down the right side to 5V
VY_TOP, VX_RIGHT, VY_BOT = 8, 1172, 780
wire(f"M{PX},{py['VCC']} H800 V{VY_TOP} H{VX_RIGHT} V{VY_BOT} H{bot['5V']} V{BOT_Y}", WIRE["VCC"][0])
for p in pins:
    dot(PX, py[p], WIRE[p][0])
for p in ["GND", "SCL", "SDA", "DC", "CS", "RST"]:
    dot(dest[p], TOP_Y, WIRE[p][0])
dot(bot["5V"], BOT_Y, WIRE["VCC"][0])

# wire name tags along the horizontal runs
for p in ["GND", "SCL", "SDA", "DC", "CS", "RST"]:
    text(700, py[p] - 6, p, 11, WIRE[p][0], "middle", "700")
text(1158, 400, "VCC", 11, WIRE["VCC"][0], "end", "700")

# ---- Button on D2 and LED on D3 ---------------------------------------------
BTN_X, LED_X, GND_Y = 720, 900, 740
# D3 -> LED (goes higher, further right), D2 -> button
wire(f"M{top['3']},{TOP_Y} V306 H{LED_X} V384", LED_WIRE, 3)
wire(f"M{top['2']},{TOP_Y} V322 H{BTN_X} V420", BTN_WIRE, 3)
dot(top["3"], TOP_Y, LED_WIRE)
dot(top["2"], TOP_Y, BTN_WIRE)

# push button symbol (vertical): terminals at 420 and 470
add(f'<line x1="{BTN_X}" y1="420" x2="{BTN_X}" y2="432" stroke="{BTN_WIRE}" stroke-width="3"/>')
add(f'<circle cx="{BTN_X}" cy="436" r="4" fill="#fff" stroke="{INK}" stroke-width="2"/>')
add(f'<circle cx="{BTN_X}" cy="462" r="4" fill="#fff" stroke="{INK}" stroke-width="2"/>')
add(f'<line x1="{BTN_X - 14}" y1="426" x2="{BTN_X - 14}" y2="472" stroke="{INK}" stroke-width="3"/>')
add(f'<line x1="{BTN_X - 14}" y1="449" x2="{BTN_X - 26}" y2="449" stroke="{INK}" stroke-width="3"/>')
add(f'<line x1="{BTN_X - 26}" y1="440" x2="{BTN_X - 26}" y2="458" stroke="{INK}" stroke-width="3"/>')
text(BTN_X + 14, 446, "Push button", 13, INK, weight="700")
text(BTN_X + 14, 463, "D2 to GND, no resistor", 12, MUTED)
text(BTN_X + 14, 479, "(internal pull-up)", 12, MUTED)
wire(f"M{BTN_X},466 V{GND_Y}", "#222222", 3)

# resistor 220 ohm + LED (vertical)
add(f'<rect x="{LED_X - 9}" y="384" width="18" height="52" rx="4" fill="#e8d3a8" stroke="{INK}" stroke-width="2"/>')
for yy, c in [(394, "#d62828"), (404, "#d62828"), (414, "#6b3e26"), (426, "#c9a227")]:
    add(f'<rect x="{LED_X - 9}" y="{yy}" width="18" height="4" fill="{c}"/>')
text(LED_X + 18, 408, "220 &#937; resistor", 13, INK, weight="700")
add(f'<line x1="{LED_X}" y1="436" x2="{LED_X}" y2="460" stroke="{LED_WIRE}" stroke-width="3"/>')
# LED triangle pointing down (anode top, cathode bottom)
add(f'<polygon points="{LED_X - 14},460 {LED_X + 14},460 {LED_X},484" fill="#fde047" stroke="{INK}" stroke-width="2"/>')
add(f'<line x1="{LED_X - 14}" y1="486" x2="{LED_X + 14}" y2="486" stroke="{INK}" stroke-width="3"/>')
add(f'<path d="M{LED_X + 16},466 l12,-8 M{LED_X + 20},476 l12,-8" stroke="#eab308" stroke-width="2"/>')
text(LED_X + 36, 470, "LED", 13, INK, weight="700")
text(LED_X + 36, 487, "long leg (+) toward D3", 12, MUTED)
text(LED_X + 36, 503, "lights while auto-cycle is on", 12, MUTED)
wire(f"M{LED_X},488 V{GND_Y}", "#222222", 3)

# shared GND return to the second GND on the power header
wire(f"M{LED_X},{GND_Y} H{bot['GND2']} V{BOT_Y}", "#222222", 3)
add(f'<circle cx="{BTN_X}" cy="{GND_Y}" r="5" fill="#222"/>')
dot(bot["GND2"], BOT_Y, "#222222")
text(810, GND_Y - 8, "GND", 11, "#222", "middle", "700")

# ---- USB to PC --------------------------------------------------------------
wire("M36,411 H22 V720 H40", "#7d8790", 6)
add(f'<rect x="40" y="690" width="230" height="64" rx="10" fill="{CARD}" stroke="#cdd3da" stroke-width="2"/>')
text(56, 716, "USB cable to the PC", 13, INK, weight="700")
text(56, 736, "power + data for the OrbDash app", 12, MUTED)

# ---- tables -------------------------------------------------------------------
TY = 812
add(f'<rect x="40" y="{TY}" width="560" height="178" rx="12" fill="{CARD}" stroke="#dde2e7"/>')
text(60, TY + 30, "Display", 15, INK, weight="700")
text(200, TY + 30, "Wire", 15, INK, weight="700")
text(330, TY + 30, "Arduino Uno", 15, INK, weight="700")
rows = [("VCC", "5V"), ("GND", "GND"), ("SCL", "D13 (SPI SCK)"), ("SDA", "D11 (SPI MOSI)"),
        ("DC", "D9"), ("CS", "D10"), ("RST", "D8")]
for i, (p, a) in enumerate(rows):
    y = TY + 52 + i * 18
    col, name = WIRE[p]
    text(60, y, p, 13, INK, weight="700")
    add(f'<rect x="200" y="{y - 10}" width="26" height="10" rx="3" fill="{col}"/>')
    text(234, y, name, 13, MUTED)
    text(330, y, a, 13, INK)

add(f'<rect x="620" y="{TY}" width="540" height="178" rx="12" fill="{CARD}" stroke="#dde2e7"/>')
text(640, TY + 30, "Optional extras", 15, INK, weight="700")
text(640, TY + 54, "Button: D2 and GND", 13, INK)
text(640, TY + 74, "LED: D3, 220 &#937; resistor, LED, GND", 13, INK)
text(640, TY + 104, "Check your display module", 13, "#b45309", weight="700")
text(640, TY + 124, "The Uno's pins are 5 V. Most GC9A01 boards have a 3.3 V", 12, MUTED)
text(640, TY + 141, "regulator and accept this. If yours is 3.3 V only, power it", 12, MUTED)
text(640, TY + 158, "from 3.3V and put a level shifter on SCL, SDA, DC, CS, RST.", 12, MUTED)

add("</svg>")
path = sys.argv[1] if len(sys.argv) > 1 else __file__.rsplit(".", 1)[0] + ".svg"
open(path, "w", encoding="utf-8").write("\n".join(out))
print("written", path)
