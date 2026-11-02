/*
 * OrbDash 1.2.0 for ESP32 - the same widgets as the Uno firmware, on
 *   OrbDash Mini: ESP32-C3 SuperMini + one 1.28" GC9A01 round display
 *   OrbDash Pro:  ESP32-S3 DevKit  + three 1.28" GC9A01 displays in a row
 * The board you compile for picks the device (C3 = Mini, S3 = Pro).
 *
 * Each frame is drawn into a 240x240 buffer in RAM and sent to the display
 * in one go, so nothing ever flickers or leaves pixels behind. The layout is
 * the same as the Uno's, so the app's preview matches.
 *
 * Wiring (all displays at 3.3 V, the ESP32's own logic level):
 *   Display | Mini (ESP32-C3) | Pro (ESP32-S3)
 *   VCC     | 3V3             | 3V3
 *   GND     | GND             | GND
 *   SCL     | GPIO4           | GPIO12   shared by all three
 *   SDA     | GPIO6           | GPIO11   shared by all three
 *   DC      | GPIO10          | GPIO9    shared by all three
 *   RST     | GPIO3           | GPIO8    shared by all three
 *   CS      | GPIO7           | GPIO10 (left), GPIO13 (middle), GPIO14 (right)
 *   Button  | BOOT (GPIO9)    | BOOT (GPIO0)    on the board, nothing to wire
 *   LED     | on-board blue   | on-board RGB    lights while auto-cycle is on
 *
 * Button (controls the middle screen on the Pro): 1 tap next widget,
 * 2 taps rotate, long press auto-cycle.
 *
 * Serial protocol: as the Uno (115200 baud, see the README), plus
 *   W|<mode>|<screen>      show a widget on screen 0-2 (screen defaults to 0)
 *   ?                      ask the board to say ORB|READY again
 *   ORB|READY|8|<version>|<screens>|<device>    MODE|<n>|<screen>
 *
 * Libraries: "Adafruit GC9A01A", "Adafruit GFX Library".
 * Boards (Arduino IDE / arduino-cli, ESP32 core 3.x):
 *   Mini: "ESP32C3 Dev Module", USB CDC On Boot: Enabled
 *   Pro:  "ESP32S3 Dev Module", plug into the board's COM/UART port
 */

#include <SPI.h>
#include <Adafruit_GFX.h>
#include <Adafruit_GC9A01A.h>

#define ORBDASH_VERSION "1.2.0"

#if CONFIG_IDF_TARGET_ESP32S3
  #define DEVICE "PRO"
  #define SCREENS 3
  #define PIN_SCK 12
  #define PIN_MOSI 11
  #define PIN_DC 9
  #define PIN_RST 8
  const int8_t PIN_CS[SCREENS] = {10, 13, 14};
  #define PIN_BTN 0
  #define PIN_RGB 48   // on-board WS2812
  #define FOCUS 1      // the screen the button and auto-cycle control
  const byte START_MODES[SCREENS] = {0, 7, 4};  // Claude 5H, GT gauge, clock
#elif CONFIG_IDF_TARGET_ESP32C3
  #define DEVICE "MINI"
  #define SCREENS 1
  #define PIN_SCK 4
  #define PIN_MOSI 6
  #define PIN_DC 10
  #define PIN_RST 3
  const int8_t PIN_CS[SCREENS] = {7};
  #define PIN_BTN 9
  #define PIN_LED 8    // on-board blue LED, lit when LOW
  #define FOCUS 0
  const byte START_MODES[SCREENS] = {0};
#else
  #error "OrbDash ESP32 firmware: build for an ESP32-C3 (Mini) or ESP32-S3 (Pro)"
#endif

#define SPI_HZ 40000000
// In the Mini and Pro cases the display's header tab points up, so the
// picture is turned 180 deg. Rotations from the app and button add to this.
#define BASE_ROT 2

// Colors (RGB565), same as the Uno firmware
#define COLOR_BG      0x0000
#define COLOR_TRACK   0x18E3
#define COLOR_DIM     0x7BEF
#define COLOR_TEXT    0xFFFF
#define COLOR_CLAUDE  0xDBAA // Claude coral: Claude widgets only
#define COLOR_BRAND   0x8AFE // Orb Violet
#define COLOR_EMBER   0xFBC8 // Ember
#define COLOR_WARN    0xFD20
#define COLOR_ALERT   0xF800
#define COLOR_LIVE    0x07E0
#define COLOR_CYAN    0x07FF
#define COLOR_PURPLE  0x9B5F
#define GT_YELLOW     0xE680
#define GT_RED_FROM   85

#define CX 120
#define CY 120
#define GAUGE_START 135
#define GAUGE_SWEEP 270
#define DEG2RAD 0.0174532925f
#define LINK_TIMEOUT_MS 15000
#define AUTO_CYCLE_MS 8000

enum { MODE_5H, MODE_TODAY, MODE_TIMER, MODE_ALL, MODE_CLOCK, MODE_SYSTEM, MODE_CUSTOM, MODE_GT, NUM_MODES };
const char *MODE_NAMES[NUM_MODES] = {"5H", "TODAY", "TIMER", "ALL", "CLOCK", "SYS", "CUSTOM", "GT"};

// -------------------------------------------------------------
// State
// -------------------------------------------------------------
struct Screen {
  Adafruit_GC9A01A *tft;
  byte mode;
  float shown;          // ring % on screen, eased toward the target
  float needle;         // GT needle %
  bool sweep;           // GT needle sweeping to full scale after opening
};
Screen scr[SCREENS];
GFXcanvas16 *canvas;

byte rotation = 0;
bool autoCycle = false;
unsigned long lastCycle = 0, lastRx = 0, lastFrame = 0;

// Data from the PC (same fields as the Uno firmware)
char blockTok[8] = "--", todayTok[8] = "--", todayMsgs[8] = "0", timeLeft[10] = "";
int gaugePct = 0, todayPct = 0;
char clockTime[6] = "--:--", clockDate[8] = "", clockDay[10] = "", clockExtra[24] = "";
byte clockSec = 0;
uint16_t clockColor = COLOR_CYAN;
byte cpuPct = 0, ramPct = 0, gpuPct = 255;  // gpu 255 = no reading
char ramText[16] = "", sysLine1[24] = "", sysLine2[24] = "";
uint16_t sysColor = COLOR_LIVE;
char custTitle[12] = "CUSTOM", custValue[9] = "--", custSub[24] = "", custLine[24] = "";
byte custPct = 0;
uint16_t custColor = COLOR_PURPLE;

void report(const char *key, int value, int screen = -1) {
  Serial.print(key);
  Serial.print('|');
  Serial.print(value);
  if (screen >= 0) { Serial.print('|'); Serial.print(screen); }
  Serial.println();
}

void sayReady() {
  Serial.print("ORB|READY|");
  Serial.print(NUM_MODES);
  Serial.print("|" ORBDASH_VERSION "|");
  Serial.print(SCREENS);
  Serial.println("|" DEVICE);
  for (byte i = 0; i < SCREENS; i++) report("MODE", scr[i].mode, i);
  report("ROT", rotation);
  report("AUTO", autoCycle);
}

bool isLinked() { return lastRx != 0 && millis() - lastRx < LINK_TIMEOUT_MS; }

// -------------------------------------------------------------
// Drawing helpers (all into the canvas)
// -------------------------------------------------------------
GFXcanvas16 &c() { return *canvas; }

void text(const char *s, int y, byte size, uint16_t color) {
  c().setTextSize(size);
  c().setTextColor(color);
  c().setCursor(CX - (int)strlen(s) * 6 * size / 2, y);
  c().print(s);
}

uint16_t mix565(uint16_t a, uint16_t b, float k) {
  int r = ((a >> 11) & 31) + (((b >> 11) & 31) - ((a >> 11) & 31)) * k;
  int g = ((a >> 5) & 63) + (((b >> 5) & 63) - ((a >> 5) & 63)) * k;
  int bl = (a & 31) + ((b & 31) - (a & 31)) * k;
  return (r << 11) | (g << 5) | bl;
}

// Radial line at `deg` (screen degrees, 0 = right, clockwise)
void ray(float deg, int r0, int r1, uint16_t color) {
  float a = deg * DEG2RAD, cs = cosf(a), sn = sinf(a);
  c().drawLine(CX + cs * r0, CY + sn * r0, CX + cs * r1, CY + sn * r1, color);
}

// Gauge ring from `from` to `to` percent of the 270 deg sweep
void ringArc(float from, float to, uint16_t color) {
  for (float d = from * 2.7f; d <= to * 2.7f + 0.01f; d += 0.5f) ray(GAUGE_START + d, 98, 110, color);
}

uint16_t levelColor(int pct) { return pct >= 90 ? COLOR_ALERT : pct >= 75 ? COLOR_WARN : COLOR_CLAUDE; }

int timerPct() {
  if (!timeLeft[0] || !strcmp(timeLeft, "IDLE") || !strcmp(timeLeft, "--")) return 0;
  int h = 0, m = 0;
  char *hp = strchr(timeLeft, 'h');
  if (hp) { h = atoi(timeLeft); m = atoi(hp + 1); } else if (strchr(timeLeft, 'm')) m = atoi(timeLeft);
  return constrain((h * 60 + m) * 100 / 300, 0, 100);
}

int targetPct(byte mode) {
  switch (mode) {
    case MODE_5H: case MODE_ALL: return gaugePct;
    case MODE_TODAY: return todayPct;
    case MODE_TIMER: return timerPct();
    case MODE_CLOCK: return clockSec * 100 / 60;
    case MODE_SYSTEM: return cpuPct;
    case MODE_CUSTOM: return custPct;
  }
  return 0;
}

uint16_t ringColor(byte mode, int p) {
  switch (mode) {
    case MODE_5H: case MODE_ALL: return levelColor(p);
    case MODE_TODAY: return p >= 90 ? COLOR_ALERT : p >= 70 ? COLOR_WARN : p >= 40 ? COLOR_CYAN : COLOR_LIVE;
    case MODE_TIMER: return p == 0 ? COLOR_TRACK : p <= 10 ? COLOR_ALERT : p <= 25 ? COLOR_WARN : COLOR_CYAN;
    case MODE_CLOCK: return clockColor;
    case MODE_SYSTEM: return p >= 85 ? COLOR_ALERT : p >= 60 ? COLOR_WARN : COLOR_LIVE;
    case MODE_CUSTOM: return custColor;
  }
  return COLOR_BRAND;
}

// -------------------------------------------------------------
// Widgets: same positions as the Uno firmware's drawStaticUI/drawReadout
// -------------------------------------------------------------
void drawWidgetText(byte mode, bool linked) {
  char line[32];
  switch (mode) {
    case MODE_5H:
      text("CLAUDE", 36, 2, COLOR_CLAUDE);
      text("5H WINDOW", 58, 1, COLOR_DIM);
      snprintf(line, sizeof(line), "%d%%", gaugePct);
      text(linked ? line : "--", 78, 4, COLOR_TEXT);
      text(linked ? blockTok : "--", 114, 2, COLOR_CLAUDE);
      text("TOKENS", 136, 1, COLOR_DIM);
      c().drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      snprintf(line, sizeof(line), "TODAY %s  %s MSG", todayTok, todayMsgs);
      text(line, 160, 1, COLOR_TEXT);
      if (!strcmp(timeLeft, "IDLE")) strcpy(line, "BLOCK IDLE");
      else snprintf(line, sizeof(line), "RESET %s", timeLeft);
      text(line, 178, 1, COLOR_DIM);
      break;

    case MODE_TODAY:
      text("CLAUDE", 36, 2, COLOR_CLAUDE);
      text("TODAY USAGE", 58, 1, COLOR_DIM);
      text(todayTok, strlen(todayTok) > 4 ? 84 : 78, strlen(todayTok) > 4 ? 3 : 4, COLOR_TEXT);
      snprintf(line, sizeof(line), "%d%% DAILY PROGRESS", todayPct);
      text(line, 116, 1, COLOR_CYAN);
      text("TOTAL TOKENS", 136, 1, COLOR_DIM);
      c().drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      snprintf(line, sizeof(line), "MESSAGES: %s SENT", todayMsgs);
      text(line, 160, 1, COLOR_TEXT);
      snprintf(line, sizeof(line), "5H BLOCK: %s (%d%%)", blockTok, gaugePct);
      text(line, 178, 1, COLOR_DIM);
      break;

    case MODE_TIMER: {
      const char *t = timeLeft[0] ? timeLeft : "IDLE";
      bool idle = !strcmp(t, "IDLE");
      text("5H TIMER", 36, 2, COLOR_CYAN);
      text("5H RESET COUNTDOWN", 58, 1, COLOR_DIM);
      text(t, strlen(t) > 5 ? 86 : 80, strlen(t) > 5 ? 2 : 3, idle ? COLOR_WARN : COLOR_CYAN);
      if (idle) strcpy(line, "SESSION IDLE");
      else snprintf(line, sizeof(line), "%d%% TIME REMAINING", timerPct());
      text(line, 116, 1, COLOR_DIM);
      text("TIME REMAINING", 136, 1, COLOR_DIM);
      c().drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      snprintf(line, sizeof(line), "5H USAGE: %d%% (%s)", gaugePct, blockTok);
      text(line, 160, 1, COLOR_TEXT);
      snprintf(line, sizeof(line), "TODAY %s  %s MSG", todayTok, todayMsgs);
      text(line, 178, 1, COLOR_DIM);
      break;
    }

    case MODE_ALL: {
      text("CLAUDE", 32, 2, COLOR_CLAUDE);
      text("ALL METRICS", 50, 1, COLOR_DIM);
      c().drawFastHLine(CX - 50, 62, 100, COLOR_TRACK);
      snprintf(line, sizeof(line), "5H TOK: %s (%d%%)", blockTok, gaugePct);
      text(line, 68, 1, COLOR_CLAUDE);
      snprintf(line, sizeof(line), "TODAY : %s TOKENS", todayTok);
      text(line, 84, 1, COLOR_TEXT);
      snprintf(line, sizeof(line), "MSGS  : %s SENT", todayMsgs);
      text(line, 100, 1, COLOR_TEXT);
      snprintf(line, sizeof(line), "RESET : %s", timeLeft);
      text(line, 116, 1, COLOR_CYAN);
      text("LINK  : LIVE (115200)", 134, 1, COLOR_LIVE);
      unsigned long s = millis() / 1000;
      snprintf(line, sizeof(line), "UPTIME: %luh %02lum", s / 3600, (s / 60) % 60);
      text(line, 150, 1, COLOR_DIM);
      c().drawFastHLine(CX - 50, 172, 100, COLOR_TRACK);
      break;
    }

    case MODE_CLOCK:
      text(clockDate, 36, 2, clockColor);
      text(clockDay, 58, 1, COLOR_DIM);
      text(clockTime, 78, 4, COLOR_TEXT);
      snprintf(line, sizeof(line), ":%02d", clockSec);
      text(line, 114, 2, clockColor);
      c().drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      text(clockExtra, 160, 1, COLOR_DIM);
      break;

    case MODE_SYSTEM:
      text("SYSTEM", 36, 2, sysColor);
      text("CPU LOAD", 58, 1, COLOR_DIM);
      snprintf(line, sizeof(line), "%d%%", cpuPct);
      text(line, 78, 4, COLOR_TEXT);
      snprintf(line, sizeof(line), "RAM %d%%", ramPct);
      text(line, 114, 2, COLOR_PURPLE);
      text(ramText, 134, 1, COLOR_DIM);
      c().drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      text(sysLine1, 160, 1, COLOR_TEXT);
      text(sysLine2, 178, 1, COLOR_DIM);
      break;

    case MODE_CUSTOM: {
      bool big = strlen(custTitle) <= 8;
      text(custTitle, big ? 36 : 40, big ? 2 : 1, custColor);
      text(custSub, 58, 1, COLOR_DIM);
      bool bigv = strlen(custValue) <= 5;
      text(custValue, bigv ? 78 : 82, bigv ? 4 : 3, COLOR_TEXT);
      c().drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      text(custLine, 160, 1, COLOR_TEXT);
      break;
    }
  }
}

void drawRingWidget(Screen &s, bool linked, float dt) {
  int target = linked ? targetPct(s.mode) : 0;
  // Clock seconds wrapping 59 -> 0: snap instead of draining the ring
  if (s.mode == MODE_CLOCK && target + 50 < s.shown) s.shown = 0;
  s.shown += (target - s.shown) * min(1.0f, dt * 6);
  uint16_t col = ringColor(s.mode, target);

  c().drawCircle(CX, CY, 118, COLOR_TRACK);
  ringArc(0, 100, COLOR_TRACK);
  if (s.shown > 0.3f) ringArc(0, s.shown, col);
  ray(GAUGE_START, 98, 113, COLOR_DIM);
  ray(GAUGE_START + GAUGE_SWEEP, 98, 113, COLOR_DIM);

  // Comet running along the ring with a short tail
  float head = (millis() % 3000) * 0.09f;  // one lap of the 270 deg ring every 3 s
  for (int k = 6; k >= 0; k--) {
    float d = head - k * 3;
    if (d < 0) d += GAUGE_SWEEP;
    ray(GAUGE_START + d, 101, 107, k < 2 ? COLOR_TEXT : COLOR_DIM);
  }

  if (linked) drawWidgetText(s.mode, true);
  else {
    text("--", 78, 4, COLOR_TEXT);
    text("RUN APP ON PC", 160, 1, COLOR_DIM);
  }

  char tag[24];
  if (autoCycle) snprintf(tag, sizeof(tag), "%s [AUTO %s]", linked ? "LIVE" : "OFF", MODE_NAMES[s.mode]);
  else snprintf(tag, sizeof(tag), "%s [%d/%d:%s]", linked ? "LIVE" : "OFF", s.mode + 1, NUM_MODES, MODE_NAMES[s.mode]);
  c().fillCircle(CX - 56, 204, 3, linked ? COLOR_LIVE : COLOR_ALERT);
  c().setTextSize(1);
  c().setTextColor(COLOR_DIM);
  c().setCursor(CX - 49, 201);
  c().print(tag);
}

// GT gauge: yellow ring with 0-10 (x10 % CPU), redline, needle, readout
void drawGT(Screen &s, bool linked, float dt) {
  if (s.sweep && s.needle >= 99) s.sweep = false;
  float target = s.sweep ? 100 : (linked ? cpuPct : 0);
  s.needle += (target - s.needle) * min(1.0f, dt * (s.sweep ? 9 : 5));

  c().fillCircle(CX, CY, 103, GT_YELLOW);
  c().fillCircle(CX, CY, 79, COLOR_BG);
  for (float d = GT_RED_FROM * 2.7f; d <= GAUGE_SWEEP; d += 0.5f) ray(GAUGE_START + d, 105, 109, COLOR_ALERT);
  for (int p = 0; p <= 100; p += 2) {
    bool red = p >= GT_RED_FROM;
    if (p % 10 == 0) {
      for (float o = -0.5f; o <= 0.5f; o += 0.5f) ray(GAUGE_START + p * 2.7f + o, 105, 118, red ? COLOR_ALERT : COLOR_TEXT);
    } else {
      ray(GAUGE_START + p * 2.7f, 111, 117, red ? COLOR_ALERT : GT_YELLOW);
    }
  }
  c().setTextSize(2);
  c().setTextColor(COLOR_BG);
  for (byte i = 0; i <= 10; i++) {
    float a = (GAUGE_START + i * 27) * DEG2RAD;
    int x = CX + cosf(a) * 91.5f - (i < 10 ? 5 : 11), y = CY + sinf(a) * 91.5f - 7;
    for (byte b = 0; b < 2; b++) { c().setCursor(x + b, y); c().print(i); }
  }

  // Needle: 3 px near the hub, 1 px at the tip
  float a = (GAUGE_START + s.needle * 2.7f) * DEG2RAD, cs = cosf(a), sn = sinf(a);
  for (int o = -1; o <= 1; o++)
    c().drawLine(CX + cs * 10 - sn * o, CY + sn * 10 + cs * o, CX + cs * 70 - sn * o, CY + sn * 70 + cs * o, COLOR_TEXT);
  c().drawLine(CX + cs * 70, CY + sn * 70, CX + cs * 100, CY + sn * 100, COLOR_TEXT);
  c().fillCircle(CX, CY, 8, 0x8410);
  c().fillCircle(CX, CY, 5, 0xC618);

  char v[24];
  if (linked) itoa(cpuPct, v, 10); else strcpy(v, "--");
  text(v, 146, 3, linked && cpuPct >= GT_RED_FROM ? COLOR_ALERT : COLOR_TEXT);
  text("CPU %", 170, 1, COLOR_DIM);
  c().setTextSize(1);
  c().setCursor(CX - 42, 180);
  for (byte k = 0; k < 2; k++) {
    byte p = k ? ramPct : gpuPct;
    c().setTextColor(COLOR_DIM);
    c().print(k ? "  RAM" : "GPU");
    if (linked && p <= 100) snprintf(v, sizeof(v), "%3d", p); else strcpy(v, " --");
    c().setTextColor(linked && p <= 100 && p >= GT_RED_FROM ? COLOR_ALERT : COLOR_TEXT);
    c().print(v);
  }
  text("x10 %", 226, 1, COLOR_DIM);
}

// No PC: violet spinner chasing round the ring, pulsing dots
void drawWaiting(byte screen) {
  unsigned long t = millis();
  float head = (t % 1000) * 0.36f;  // one turn a second
  for (float d = 0; d < 360; d += 0.5f) ray(d, 98, 110, COLOR_TRACK);
  for (float d = 0; d <= 80; d += 0.5f) ray(head - d, 98, 110, d < 6 ? COLOR_TEXT : mix565(COLOR_BRAND, COLOR_TRACK, d / 90));
  c().drawCircle(CX, CY, 118, COLOR_TRACK);
  text("ORBDASH", 70, 3, COLOR_BRAND);
  text("WAITING FOR PC", 108, 1, COLOR_TEXT);
  for (byte i = 0; i < 3; i++) c().fillCircle(CX - 14 + i * 14, 132, 3, (t / 260) % 4 == i ? COLOR_EMBER : COLOR_TRACK);
  text("START THE ORBDASH APP", 156, 1, COLOR_DIM);
  char who[24];
  if (SCREENS > 1) snprintf(who, sizeof(who), DEVICE " %d/%d  v" ORBDASH_VERSION, screen + 1, SCREENS);
  else snprintf(who, sizeof(who), DEVICE "  v" ORBDASH_VERSION);
  text(who, 170, 1, COLOR_DIM);
}

void push(byte i) {
  scr[i].tft->drawRGBBitmap(0, 0, canvas->getBuffer(), 240, 240);
}

// -------------------------------------------------------------
// Boot animation (~2.6 s): rings, ORBDASH typing, a violet-to-ember sweep
// -------------------------------------------------------------
void readSerial();
void checkButton();

void bootAnimation() {
  unsigned long t0 = millis();
  for (;;) {
    float t = (millis() - t0) / 1000.0f;
    if (t > 2.6f) break;
    c().fillScreen(COLOR_BG);
    for (int w = 0; w < 2; w++) {
      int r = (int)((t - w * 0.25f) * 220) % 160;
      if (t < 0.9f && r > 0 && r < 119) c().drawCircle(CX, CY, r, w ? COLOR_DIM : COLOR_BRAND);
    }
    if (t > 0.5f) {
      char word[8] = "ORBDASH";
      int n = constrain((int)((t - 0.5f) / 0.07f), 0, 7);
      word[n] = 0;
      c().setTextSize(3);
      c().setTextColor(COLOR_BRAND);
      c().setCursor(CX - 63, 100);
      c().print(word);
    }
    if (t > 1.1f) text("DESK DASHBOARD", 132, 1, COLOR_DIM);
    if (t > 1.3f) {
      float p = constrain((t - 1.3f) / 0.8f, 0.0f, 1.0f) * 100;
      ringArc(0, 100, COLOR_TRACK);
      ringArc(0, p, p < 75 ? COLOR_BRAND : COLOR_EMBER);
    }
    for (byte i = 0; i < SCREENS; i++) { push(i); readSerial(); }
    checkButton();
  }
}

// -------------------------------------------------------------
// Controls
// -------------------------------------------------------------
void setLed(bool on) {
#ifdef PIN_LED
  digitalWrite(PIN_LED, on ? LOW : HIGH);
#endif
#ifdef PIN_RGB
  rgbLedWrite(PIN_RGB, on ? 40 : 0, 0, on ? 80 : 0);
#endif
}

void setAutoCycle(bool on) {
  if (on != autoCycle) report("AUTO", on);
  autoCycle = on;
  lastCycle = millis();
  setLed(on);
}

void showMode(byte screen, byte mode) {
  if (screen >= SCREENS) return;
  Screen &s = scr[screen];
  mode %= NUM_MODES;
  if (mode == MODE_GT && s.mode != MODE_GT) { s.needle = 0; s.sweep = true; }
  s.mode = mode;
  report("MODE", mode, screen);
}

void setRotation(byte r) {
  rotation = r % 4;
  for (byte i = 0; i < SCREENS; i++) scr[i].tft->setRotation((rotation + BASE_ROT) % 4);
  report("ROT", rotation);
}

void handleCommand(char cmd) {
  if (cmd == 'm' || cmd == 'M') { setAutoCycle(false); showMode(FOCUS, scr[FOCUS].mode + 1); }
  else if (cmd == 'r' || cmd == 'R') setRotation(rotation + 1);
  else if (cmd == 'a' || cmd == 'A') setAutoCycle(!autoCycle);
  else if (cmd == '?') sayReady();
  else if (cmd >= '1' && cmd < '1' + NUM_MODES) { setAutoCycle(false); showMode(FOCUS, cmd - '1'); }
}

// Button: 1 tap next widget, 2 taps rotate, long press auto-cycle
void checkButton() {
  static int last = HIGH, state = HIGH;
  static unsigned long debounce = 0, pressed = 0, released = 0;
  static bool longDone = false;
  static byte clicks = 0;
  int reading = digitalRead(PIN_BTN);
  if (reading != last) debounce = millis();
  last = reading;
  if (millis() - debounce > 40 && reading != state) {
    state = reading;
    if (state == LOW) { pressed = millis(); longDone = false; }
    else if (!longDone) { clicks++; released = millis(); }
  }
  if (state == LOW && !longDone && millis() - pressed > 1000) {
    longDone = true;
    clicks = 0;
    setAutoCycle(!autoCycle);
  }
  if (clicks && state == HIGH && millis() - released > 350) {
    if (clicks >= 2) setRotation(rotation + 1);
    else { setAutoCycle(false); showMode(FOCUS, scr[FOCUS].mode + 1); }
    clicks = 0;
  }
}

// -------------------------------------------------------------
// Serial
// -------------------------------------------------------------
char rxBuf[96];
byte rxLen = 0;

void copyField(char *dst, size_t size, const char *src) {
  if (!src) return;
  strncpy(dst, src, size - 1);
  dst[size - 1] = 0;
}

void parseLine() {
  if (rxBuf[1] != '|') return;
  char type = rxBuf[0];
  strtok(rxBuf, "|");
  char *f;
  switch (type) {
    case 'U':
      copyField(blockTok, sizeof(blockTok), strtok(NULL, "|"));
      if ((f = strtok(NULL, "|"))) gaugePct = constrain(atoi(f), 0, 100);
      copyField(todayTok, sizeof(todayTok), strtok(NULL, "|"));
      copyField(todayMsgs, sizeof(todayMsgs), strtok(NULL, "|"));
      copyField(timeLeft, sizeof(timeLeft), strtok(NULL, "|"));
      if ((f = strtok(NULL, "|"))) todayPct = constrain(atoi(f), 0, 100);
      break;
    case 'C':
      copyField(clockTime, sizeof(clockTime), strtok(NULL, "|"));
      if ((f = strtok(NULL, "|"))) clockSec = constrain(atoi(f), 0, 59);
      copyField(clockDate, sizeof(clockDate), strtok(NULL, "|"));
      copyField(clockDay, sizeof(clockDay), strtok(NULL, "|"));
      copyField(clockExtra, sizeof(clockExtra), strtok(NULL, "|"));
      if ((f = strtok(NULL, "|"))) clockColor = strtoul(f, NULL, 16);
      break;
    case 'S':
      if ((f = strtok(NULL, "|"))) cpuPct = constrain(atoi(f), 0, 100);
      if ((f = strtok(NULL, "|"))) ramPct = constrain(atoi(f), 0, 100);
      copyField(ramText, sizeof(ramText), strtok(NULL, "|"));
      copyField(sysLine1, sizeof(sysLine1), strtok(NULL, "|"));
      copyField(sysLine2, sizeof(sysLine2), strtok(NULL, "|"));
      if ((f = strtok(NULL, "|"))) sysColor = strtoul(f, NULL, 16);
      break;
    case 'M':
      copyField(custTitle, sizeof(custTitle), strtok(NULL, "|"));
      copyField(custValue, sizeof(custValue), strtok(NULL, "|"));
      copyField(custSub, sizeof(custSub), strtok(NULL, "|"));
      copyField(custLine, sizeof(custLine), strtok(NULL, "|"));
      if ((f = strtok(NULL, "|"))) custPct = constrain(atoi(f), 0, 100);
      if ((f = strtok(NULL, "|"))) custColor = strtoul(f, NULL, 16);
      break;
    case 'G':
      if ((f = strtok(NULL, "|"))) cpuPct = constrain(atoi(f), 0, 100);
      if ((f = strtok(NULL, "|"))) gpuPct = isdigit(f[0]) ? constrain(atoi(f), 0, 100) : 255;
      if ((f = strtok(NULL, "|"))) ramPct = constrain(atoi(f), 0, 100);
      break;
    case 'W': {
      char *m = strtok(NULL, "|");
      char *sc = strtok(NULL, "|");
      byte screen = sc ? atoi(sc) : 0;
      if (m) {
        setAutoCycle(false);
        if (screen < SCREENS && atoi(m) != scr[screen].mode) showMode(screen, atoi(m));
      }
      break;
    }
    case 'R':
      if ((f = strtok(NULL, "|")) && atoi(f) != rotation) setRotation(atoi(f));
      break;
    case 'A':
      if ((f = strtok(NULL, "|"))) setAutoCycle(atoi(f) != 0);
      break;
    default:
      return;
  }
  lastRx = millis();
  if (!lastRx) lastRx = 1;
}

void readSerial() {
  while (Serial.available()) {
    char ch = Serial.read();
    if (ch == '\r') continue;
    if (ch == '\n') {
      rxBuf[rxLen] = 0;
      if (rxLen == 1) handleCommand(rxBuf[0]);
      else if (rxLen > 1) parseLine();
      rxLen = 0;
    } else if (rxLen < sizeof(rxBuf) - 1) {
      rxBuf[rxLen++] = ch;
    } else {
      rxLen = 0;
    }
  }
}

// -------------------------------------------------------------
void setup() {
  Serial.setRxBufferSize(1024);
  Serial.begin(115200);
  pinMode(PIN_BTN, INPUT_PULLUP);
#ifdef PIN_LED
  pinMode(PIN_LED, OUTPUT);
#endif
  setLed(false);

  // One reset for all displays (they share RST), then start each
  pinMode(PIN_RST, OUTPUT);
  digitalWrite(PIN_RST, LOW);
  delay(20);
  digitalWrite(PIN_RST, HIGH);
  delay(150);
  SPI.begin(PIN_SCK, -1, PIN_MOSI, -1);
  for (byte i = 0; i < SCREENS; i++) {
    pinMode(PIN_CS[i], OUTPUT);
    digitalWrite(PIN_CS[i], HIGH);  // keep the others quiet while one starts
  }
  for (byte i = 0; i < SCREENS; i++) {
    scr[i].tft = new Adafruit_GC9A01A(&SPI, PIN_CS[i], PIN_DC, -1);
    scr[i].tft->begin(SPI_HZ);
    scr[i].tft->setRotation((rotation + BASE_ROT) % 4);
    scr[i].mode = START_MODES[i];
    scr[i].shown = 0;
    scr[i].needle = 0;
    scr[i].sweep = scr[i].mode == MODE_GT;
  }
  canvas = new GFXcanvas16(240, 240);

  bootAnimation();
  sayReady();
  lastFrame = millis();
}

void loop() {
  readSerial();
  checkButton();
  if (autoCycle && millis() - lastCycle >= AUTO_CYCLE_MS) {
    lastCycle = millis();
    showMode(FOCUS, scr[FOCUS].mode + 1);
  }

  unsigned long now = millis();
  float dt = (now - lastFrame) / 1000.0f;
  lastFrame = now;
  bool linked = isLinked();
  for (byte i = 0; i < SCREENS; i++) {
    c().fillScreen(COLOR_BG);
    if (!linked) drawWaiting(i);
    else if (scr[i].mode == MODE_GT) drawGT(scr[i], linked, dt);
    else drawRingWidget(scr[i], linked, dt);
    push(i);
    readSerial();
  }

  // About 30 frames a second at most
  while (millis() - now < 33) { readSerial(); checkButton(); delay(1); }
}
