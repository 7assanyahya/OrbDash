/*
 * OrbDash - round desk dashboard on a 1.28" GC9A01 TFT (Arduino Uno/Nano)
 * Claude usage, clock, PC stats and custom widgets, fed by the OrbDash app
 *
 * Pinout Wiring Guide:
 * -------------------------------------------------------------------------
 * Component    | Pin Name      | Wire Color    | Arduino Uno/Nano Pin
 * -------------------------------------------------------------------------
 * Display      | VCC           | Red           | 5V
 * Display      | GND           | Black         | GND
 * Display      | SCL           | Magenta/Purple| Pin 13 (Hardware SPI SCK)
 * Display      | SDA           | Orange/Brown  | Pin 11 (Hardware SPI MOSI)
 * Display      | DC            | Green         | Pin 9
 * Display      | CS            | Yellow        | Pin 10
 * Display      | RST           | Cyan/Blue     | Pin 8
 * -------------------------------------------------------------------------
 * Push Button  | Terminal 1    | Any           | Pin 2
 * Push Button  | Terminal 2    | Any           | GND (Ground)
 *              (Uses internal pull-up on Pin 2 - NO resistor required)
 * -------------------------------------------------------------------------
 * Auto-Rot LED | Anode (+)     | Resistor 220Ω | Pin 3
 *              | Cathode (-)   | Direct to GND | GND (Ground)
 *              (LED turns ON when auto-cycle is active)
 * -------------------------------------------------------------------------
 *
 * Button Controls:
 * - 1 Tap:           Cycle to next display mode (1 -> 2 -> ... -> 7 -> 1)
 * - 2 Taps:          Rotate screen by 90 degrees (0° -> 90° -> 180° -> 270°)
 * - Long Press (>1s): Toggle AUTO-CYCLE (LED lights up, rotates modes every 8s)
 * - Serial Commands:  Send 'm' (mode), 'r' (rotate 90°), 'a' (auto-cycle), '1'-'8' (modes)
 *
 * Mode-Specific Ring Behavior (Gauge ring reflects the active mode's data):
 * 1. [5H DIAL]   - Ring shows 5H block token / limit percentage (Coral -> Amber -> Red)
 * 2. [TODAY]     - Ring shows Today's cumulative usage progress (Live Green -> Cyan -> Alert)
 * 3. [TIMER]     - Ring shows 5H window time remaining countdown clock (Glowing Cyan -> Amber)
 * 4. [ALL/DASH]  - Ring shows primary 5H load dial framing the full metrics dashboard
 * 5. [CLOCK]     - Ring shows seconds of the current minute
 * 6. [SYSTEM]    - Ring shows PC CPU load (Green -> Amber -> Red)
 * 7. [CUSTOM]    - Anything the PC app sends: title, value, subtitle, ring %, color
 * 8. [GT]        - Race-car style tachometer: needle = CPU, plus GPU and RAM bars
 *
 * Serial protocol (115200 baud, '|' separated, one message per line):
 *   PC -> board
 *     U|<5h tok>|<5h %>|<today tok>|<msgs>|<time left>|<today %>   Claude usage
 *     C|<HH:MM>|<SS>|<date>|<weekday>|<extra>[|<color>]             Clock
 *     S|<cpu %>|<ram %>|<ram text>|<line 1>|<line 2>[|<color>]      System stats
 *     M|<title>|<value>|<subtitle>|<line>|<ring %>|<color 565 hex>  Custom widget
 *     G|<cpu %>|<gpu %>|<ram %>                                     GT gauge (gpu "-" = none)
 *     W|<mode 0-7>   show a mode      R|<0-3>  set rotation      A|<0/1>  auto-cycle
 *   board -> PC
 *     ORB|READY|<modes>    MODE|<n>   ROT|<n>   AUTO|<0/1>
 *   Fields must not be empty (send a space instead).
 *
 * Libraries Required:
 * - "Adafruit GC9A01A"
 * - "Adafruit GFX Library"
 */

#include <SPI.h>
#include <Adafruit_GFX.h>
#include <Adafruit_GC9A01A.h>

#define TFT_RST      8
#define TFT_DC       9
#define TFT_CS       10
#define BUTTON_PIN   2
#define LED_AUTO_PIN 3  // LED: Auto-Cycle status

Adafruit_GC9A01A tft(TFT_CS, TFT_DC, TFT_RST);

// Colors in 16-bit 565 format
#define COLOR_BG      0x0000 // Black
#define COLOR_TRACK   0x18E3 // Dark gray gauge track
#define COLOR_DIM     0x7BEF // Mid gray labels
#define COLOR_TEXT    0xFFFF // White
#define COLOR_CLAUDE  0xDBAA // Claude coral (#D97757): Claude widgets only
#define COLOR_BRAND   0x8AFE // OrbDash Orb Violet (#8B5CF6)
#define COLOR_EMBER   0xFBC8 // OrbDash Ember (#FF7A45)
#define COLOR_WARN    0xFD20 // Amber
#define COLOR_ALERT   0xF800 // Red
#define COLOR_LIVE    0x07E0 // Green
#define COLOR_CYAN    0x07FF // Cyan
#define COLOR_PURPLE  0x9B5F // Soft purple

#define CX 120
#define CY 120

// Gauge ring: 270 deg sweep, opening at the bottom
#define GAUGE_START 135
#define GAUGE_SWEEP 270
#define GAUGE_R_IN  98
#define GAUGE_R_OUT 110

// Orbiting comet
#define ORBIT_R     104  // on the gauge ring itself, so it never touches text
#define ORBIT_TRAIL 10

#define LINK_TIMEOUT_MS 15000
#define DEG2RAD 0.0174532925

// Display Modes
enum DisplayMode {
  MODE_5H_DIAL   = 0,
  MODE_TODAY     = 1,
  MODE_TIMER     = 2,
  MODE_DASHBOARD = 3,
  MODE_CLOCK     = 4,
  MODE_SYSTEM    = 5,
  MODE_CUSTOM    = 6,
  MODE_GT        = 7,
  NUM_MODES      = 8
};

// GT gauge: own face instead of the ring, needle shows CPU
#define GT_YELLOW     0xE680 // Ring colour
#define GT_BAND_IN    80     // yellow ring inner/outer radius
#define GT_BAND_OUT   103
#define GT_NEEDLE_LEN 100    // needle tip radius (inside the ring)
#define GT_RED_FROM   85     // redline starts here (%)

bool booting = true;  // mode/rotation changes during boot only update state

byte currentMode = MODE_5H_DIAL;
bool autoCycle = false;
unsigned long lastModeCycleTime = 0;
#define AUTO_CYCLE_INTERVAL 8000 // 8s per mode

// Button handling
int btnLastReading = HIGH;
int btnState = HIGH;
unsigned long lastDebounceTime = 0;
unsigned long btnPressTime = 0;
bool longPressTriggered = false;
byte clickCount = 0;
unsigned long lastReleaseTime = 0;
#define DEBOUNCE_DELAY 40
#define LONG_PRESS_DELAY 1000
#define MULTI_CLICK_WINDOW 350

byte screenRotation = 0; // 0 = 0°, 1 = 90°, 2 = 180°, 3 = 270°

// Claude usage data
char rxBuf[80];
byte rxLen = 0;
char blockTok[8] = "--";
char todayTok[8] = "--";
char todayMsgs[8] = "0";
char timeLeft[10] = "";
int gaugePct = 0;
int todayPct = 0;
unsigned long lastRx = 0;

// Clock widget
char clockTime[6] = "--:--";
byte clockSec = 0;
char clockDate[8] = "";
char clockDay[10] = "";
char clockExtra[24] = "";
uint16_t clockColor = COLOR_CYAN;

// System widget
byte cpuPct = 0;
byte ramPct = 0;
char ramText[16] = "";
char sysLine1[24] = "";
char sysLine2[24] = "";
uint16_t sysColor = COLOR_LIVE;

// GT widget (CPU and RAM are shared with the system widget)
byte gpuPct = 255;    // 255 = the PC has no GPU reading
int gtShown = -1;     // needle position in %, -1 = not drawn
bool gtSweep = false; // needle sweeps to full scale when the widget opens

// Custom widget
char custTitle[12] = "CUSTOM";
char custValue[9] = "--";
char custSub[24] = "";
char custLine[24] = "";
byte custPct = 0;
uint16_t custColor = COLOR_PURPLE;

// Display state
bool textDirty = true;
bool wasLinked = false;
bool waiting = false;   // no PC: the waiting screen replaces the widgets
int waitHead = 0;       // spinner head, degrees
int shownPct = 0;
uint16_t ringColor = COLOR_CLAUDE;  // color of the filled part of the ring
byte pctTextLen = 0;
int orbitAngle = 0;
int frameCount = 0;

// Forward declarations
void checkButton();
void rotateScreen();
void setAutoCycle(bool enable);
void switchMode(byte newMode);
void nextMode();
void handleCommand(char cmd);
void readSerial();
bool isLinked();
int getTargetPctForMode();
uint16_t getRingColor(int pct);
int getTimerPct();
int calculateTodayPct();
void drawStaticUI(bool withFrame = true);
void drawReadout(bool linked);
void drawLinkStatus(bool linked);
void drawPercent(int pct);
void animateGauge(int target);
void updateOrbit();
void drawArc(int fromT, int toT, uint16_t color);
void drawTick(int deg, uint16_t color);
uint16_t levelColor(int pct);
void printCentered(const char *s, int y, byte size, uint16_t color);
void printCenteredP(const __FlashStringHelper *s, int y, byte size, uint16_t color);
void printBand(const char *s, int y, byte size, uint16_t color, int bandW);
void bootAnimation();
void bootWait(int ms);
void getUptimeStr(char *buf, size_t sz);
void drawGtFace();
void gtUpdate(bool linked);
void gtReadout(bool linked);
void setWaiting(bool on);
void drawWaitScreen(bool withFrame);
void waitAnimation();

void setup() {
  Serial.begin(115200);

  pinMode(BUTTON_PIN, INPUT_PULLUP);
  pinMode(LED_AUTO_PIN, OUTPUT);
  digitalWrite(LED_AUTO_PIN, LOW);

  tft.begin();
  tft.setRotation(screenRotation);
  tft.fillScreen(COLOR_BG);

  bootAnimation();
  booting = false;
  tft.setRotation(screenRotation);  // in case it was changed while booting
  tft.fillScreen(COLOR_BG);
  setWaiting(!isLinked());
  wasLinked = isLinked();

  Serial.print(F("ORB|READY|"));
  Serial.println(NUM_MODES);
  report(F("MODE"), currentMode);
  report(F("ROT"), screenRotation);
  report(F("AUTO"), autoCycle);
}

// Tell the PC app about state changes (button presses, auto-cycle)
void report(const __FlashStringHelper *key, int value) {
  Serial.print(key);
  Serial.print('|');
  Serial.println(value);
}

// -------------------------------------------------------------
// Boot Animation (~5 s)
// -------------------------------------------------------------
void bootWait(int ms) {
  unsigned long end = millis() + ms;
  while ((long)(end - millis()) > 0) {
    readSerial();
    checkButton();
  }
}

void typeCentered(const char *s, int y, byte size, uint16_t color, int perChar) {
  int x = CX - strlen(s) * 6 * size / 2;
  tft.setTextSize(size);
  tft.setTextColor(color, COLOR_BG);
  for (byte i = 0; s[i]; i++) {
    tft.fillRect(x, y, 6 * size - size, 8 * size, COLOR_TEXT);
    bootWait(perChar / 2);
    tft.setCursor(x, y);
    tft.print(s[i]);
    x += 6 * size;
    bootWait(perChar / 2);
  }
}

void bootAnimation() {
  for (byte wave = 0; wave < 2; wave++) {
    for (int r = 4; r <= 140; r += 8) {
      if (r <= 118) tft.drawCircle(CX, CY, r, wave ? COLOR_DIM : COLOR_BRAND);
      if (r - 24 > 0) tft.drawCircle(CX, CY, r - 24, COLOR_BG);
      bootWait(12);
    }
  }

  typeCentered("ORBDASH", 100, 3, COLOR_BRAND, 100);
  bootWait(100);
  typeCentered("DESK DASHBOARD", 132, 1, COLOR_DIM, 40);
  bootWait(250);

  for (int t = 0; t < GAUGE_SWEEP * 10; t += 90) {
    drawArc(t, min(t + 90, GAUGE_SWEEP * 10), COLOR_TRACK);
    bootWait(5);
  }
  for (int p = 0; p < 100; p += 2) {
    drawArc(p * GAUGE_SWEEP / 10, (p + 2) * GAUGE_SWEEP / 10, p < 75 ? COLOR_BRAND : COLOR_EMBER);
    bootWait(10);
  }
  bootWait(300);
  for (int p = 100; p > 0; p -= 4) {
    drawArc((p - 4) * GAUGE_SWEEP / 10, p * GAUGE_SWEEP / 10, COLOR_TRACK);
    bootWait(8);
  }
  bootWait(300);

  for (int r = 0; r <= 122; r += 8) {
    tft.fillCircle(CX, CY, r, COLOR_BG);
    bootWait(8);
  }
  tft.fillScreen(COLOR_BG);
}

// -------------------------------------------------------------
// Main Loop
// -------------------------------------------------------------
void loop() {
  checkButton();
  readSerial();

  if (autoCycle && (millis() - lastModeCycleTime >= AUTO_CYCLE_INTERVAL)) {
    lastModeCycleTime = millis();
    nextMode();
  }

  bool linked = isLinked();
  if (linked != wasLinked) {
    wasLinked = linked;
    textDirty = true;
    setWaiting(!linked);
  }

  if (waiting) {
    waitAnimation();
  } else if (currentMode == MODE_GT) {
    gtUpdate(linked);
  } else {
    int target = linked ? getTargetPctForMode() : 0;
    animateGauge(target);
    updateOrbit();
  }

  if (textDirty && !waiting) {
    drawReadout(linked);
    textDirty = false;
  }

  if (frameCount % 15 == 0) {
    drawLinkStatus(linked);
  }

  frameCount++;
  if (frameCount > 1000) frameCount = 0;

  // Frame delay that keeps draining serial (a plain delay() lets bursts overflow)
  unsigned long frameEnd = millis() + 30;
  while ((long)(frameEnd - millis()) > 0) pumpSerial();
}

// -------------------------------------------------------------
// Mode Data & Ring Calculations
// -------------------------------------------------------------
int getTimerPct() {
  if (strcmp(timeLeft, "IDLE") == 0 || timeLeft[0] == '\0' || strcmp(timeLeft, "--") == 0) {
    return 0;
  }
  int h = 0, m = 0;
  char *hPtr = strchr(timeLeft, 'h');
  if (hPtr) {
    h = atoi(timeLeft);
    m = atoi(hPtr + 1);
  } else {
    char *mPtr = strchr(timeLeft, 'm');
    if (mPtr) {
      m = atoi(timeLeft);
    }
  }
  int totalMins = h * 60 + m;
  int pct = constrain((int)((totalMins * 100L) / 300L), 0, 100);
  return pct;
}

int calculateTodayPct() {
  if (strcmp(todayTok, "--") == 0 || todayTok[0] == '\0') return 0;
  float val = atof(todayTok);
  char suffix = 0;
  for (byte i = 0; todayTok[i]; i++) {
    char c = toupper(todayTok[i]);
    if (c == 'K' || c == 'M' || c == 'B') { suffix = c; break; }
  }
  long tokens = 0;
  if (suffix == 'K') tokens = (long)(val * 1000.0);
  else if (suffix == 'M') tokens = (long)(val * 1000000.0);
  else if (suffix == 'B') tokens = (long)(val * 1000000000.0);
  else tokens = (long)val;

  if (tokens <= 0) return 0;
  int pct = (int)((tokens * 100L) / 2000000L);
  return constrain(pct, 0, 100);
}

int getTargetPctForMode() {
  switch (currentMode) {
    case MODE_5H_DIAL:
      return gaugePct;
    case MODE_TODAY:
      return todayPct > 0 ? todayPct : calculateTodayPct();
    case MODE_TIMER:
      return getTimerPct();
    case MODE_DASHBOARD:
      return gaugePct;
    case MODE_CLOCK:
      return clockSec * 100 / 60;
    case MODE_SYSTEM:
      return cpuPct;
    case MODE_CUSTOM:
      return custPct;
  }
  return 0;
}

uint16_t getRingColor(int pct) {
  switch (currentMode) {
    case MODE_5H_DIAL:
    case MODE_DASHBOARD:
      return levelColor(pct);

    case MODE_TODAY:
      if (pct >= 90) return COLOR_ALERT;
      if (pct >= 70) return COLOR_WARN;
      if (pct >= 40) return COLOR_CYAN;
      return COLOR_LIVE;

    case MODE_TIMER:
      if (pct == 0)  return COLOR_TRACK;
      if (pct <= 10) return COLOR_ALERT;
      if (pct <= 25) return COLOR_WARN;
      return COLOR_CYAN;

    case MODE_CLOCK:
      return clockColor;

    case MODE_SYSTEM:
      if (pct >= 85) return COLOR_ALERT;
      if (pct >= 60) return COLOR_WARN;
      return COLOR_LIVE;

    case MODE_CUSTOM:
      return custColor;
  }
  return COLOR_CLAUDE;
}

// -------------------------------------------------------------
// Button & Controls
// -------------------------------------------------------------
void setAutoCycle(bool enable) {
  bool changed = autoCycle != enable;
  autoCycle = enable;
  digitalWrite(LED_AUTO_PIN, autoCycle ? HIGH : LOW);
  lastModeCycleTime = millis();
  if (changed) report(F("AUTO"), autoCycle);
  if (booting) return;
  textDirty = true;
  drawLinkStatus(isLinked());
}

void checkButton() {
  int reading = digitalRead(BUTTON_PIN);

  if (reading != btnLastReading) {
    lastDebounceTime = millis();
  }

  if ((millis() - lastDebounceTime) > DEBOUNCE_DELAY) {
    if (reading != btnState) {
      btnState = reading;

      if (btnState == LOW) {
        btnPressTime = millis();
        longPressTriggered = false;
      } else {
        if (!longPressTriggered) {
          clickCount++;
          lastReleaseTime = millis();
          if (clickCount >= 2) {
            rotateScreen();
            clickCount = 0;
          }
        }
      }
    } else if (btnState == LOW && !longPressTriggered) {
      if ((millis() - btnPressTime) >= LONG_PRESS_DELAY) {
        longPressTriggered = true;
        clickCount = 0;
        setAutoCycle(!autoCycle);
      }
    }
  }

  btnLastReading = reading;

  if (clickCount > 0 && btnState == HIGH) {
    if ((millis() - lastReleaseTime) > MULTI_CLICK_WINDOW) {
      if (clickCount == 1) {
        // 1 Tap: Next mode
        setAutoCycle(false);
        nextMode();
      }
      clickCount = 0;
    }
  }
}

void setRotation(byte rot) {
  screenRotation = rot % 4;
  report(F("ROT"), screenRotation);
  if (booting) return;
  tft.setRotation(screenRotation);
  clearScreen();
  shownPct = 0;
  pctTextLen = 0;
  gtShown = -1;
  drawStaticUI();
  textDirty = true;
}

void rotateScreen() {
  setRotation(screenRotation + 1);
}

void switchMode(byte newMode) {
  byte oldMode = currentMode;
  currentMode = newMode % NUM_MODES;
  report(F("MODE"), currentMode);
  if (booting) return;
  pctTextLen = 0;
  if (oldMode == MODE_GT || currentMode == MODE_GT) {
    // The GT face has no ring: redraw the whole screen
    clearScreen();
    gtShown = -1;
    gtSweep = true;
    drawStaticUI();
  } else {
    // Keep the ring: only the inside is cleared, and the ring then animates
    // from its old value to the new widget's value
    clearInner();
    drawStaticUI(false);
  }
  textDirty = true;
}

void nextMode() {
  switchMode(currentMode + 1);
}

void handleCommand(char cmd) {
  if (cmd == 'm' || cmd == 'M') {
    setAutoCycle(false);
    nextMode();
  } else if (cmd == 'r' || cmd == 'R') {
    rotateScreen();
  } else if (cmd == 'a' || cmd == 'A') {
    setAutoCycle(!autoCycle);
  } else if (cmd >= '1' && cmd < '1' + NUM_MODES) {
    setAutoCycle(false);
    switchMode(cmd - '1');
  }
}

// -------------------------------------------------------------
// Serial Link: parse "U|..." usage messages
// -------------------------------------------------------------
void copyField(char *dst, size_t size, const char *src) {
  if (!src) return;
  strncpy(dst, src, size - 1);
  dst[size - 1] = '\0';
}

void parseLine() {
  if (rxBuf[0] == 'U' && rxBuf[1] == '|') {
    strtok(rxBuf, "|");
    copyField(blockTok, sizeof(blockTok), strtok(NULL, "|"));
    char *pct = strtok(NULL, "|");
    if (pct) gaugePct = constrain(atoi(pct), 0, 100);
    copyField(todayTok, sizeof(todayTok), strtok(NULL, "|"));
    copyField(todayMsgs, sizeof(todayMsgs), strtok(NULL, "|"));
    copyField(timeLeft, sizeof(timeLeft), strtok(NULL, "|"));
    char *todPct = strtok(NULL, "|");
    if (todPct) todayPct = constrain(atoi(todPct), 0, 100);
    else todayPct = calculateTodayPct();

    markRx(true);
    return;
  }

  if (rxBuf[1] != '|') return;
  char type = rxBuf[0];
  strtok(rxBuf, "|");

  switch (type) {
    case 'C': {
      copyField(clockTime, sizeof(clockTime), strtok(NULL, "|"));
      char *sec = strtok(NULL, "|");
      if (sec) clockSec = constrain(atoi(sec), 0, 59);
      copyField(clockDate, sizeof(clockDate), strtok(NULL, "|"));
      copyField(clockDay, sizeof(clockDay), strtok(NULL, "|"));
      copyField(clockExtra, sizeof(clockExtra), strtok(NULL, "|"));
      char *col = strtok(NULL, "|");
      if (col) clockColor = strtoul(col, NULL, 16);
      markRx(currentMode == MODE_CLOCK);
      break;
    }
    case 'S': {
      char *f = strtok(NULL, "|");
      if (f) cpuPct = constrain(atoi(f), 0, 100);
      f = strtok(NULL, "|");
      if (f) ramPct = constrain(atoi(f), 0, 100);
      copyField(ramText, sizeof(ramText), strtok(NULL, "|"));
      copyField(sysLine1, sizeof(sysLine1), strtok(NULL, "|"));
      copyField(sysLine2, sizeof(sysLine2), strtok(NULL, "|"));
      char *col = strtok(NULL, "|");
      if (col) {
        uint16_t c = strtoul(col, NULL, 16);
        if (c != sysColor && currentMode == MODE_SYSTEM && !booting) printCenteredP(F("SYSTEM"), 36, 2, c);
        sysColor = c;
      }
      markRx(currentMode == MODE_SYSTEM);
      break;
    }
    case 'M': {
      char oldTitle[sizeof(custTitle)];
      strcpy(oldTitle, custTitle);
      copyField(custTitle, sizeof(custTitle), strtok(NULL, "|"));
      copyField(custValue, sizeof(custValue), strtok(NULL, "|"));
      copyField(custSub, sizeof(custSub), strtok(NULL, "|"));
      copyField(custLine, sizeof(custLine), strtok(NULL, "|"));
      char *f = strtok(NULL, "|");
      if (f) custPct = constrain(atoi(f), 0, 100);
      f = strtok(NULL, "|");
      if (f) custColor = strtoul(f, NULL, 16);
      // Title may switch text size, so clear its box when it changes
      if (currentMode == MODE_CUSTOM && strcmp(oldTitle, custTitle) != 0 && !booting) {
        tft.fillRect(CX - 48, 36, 96, 16, COLOR_BG);
      }
      markRx(currentMode == MODE_CUSTOM);
      break;
    }
    case 'G': {
      char *f = strtok(NULL, "|");
      if (f) cpuPct = constrain(atoi(f), 0, 100);
      f = strtok(NULL, "|");
      if (f) gpuPct = isdigit(f[0]) ? constrain(atoi(f), 0, 100) : 255;
      f = strtok(NULL, "|");
      if (f) ramPct = constrain(atoi(f), 0, 100);
      markRx(currentMode == MODE_GT);
      break;
    }
    case 'W': {
      char *f = strtok(NULL, "|");
      if (f) {
        setAutoCycle(false);
        if (atoi(f) != currentMode) switchMode(atoi(f));
      }
      markRx(false);
      break;
    }
    case 'R': {
      char *f = strtok(NULL, "|");
      if (f && atoi(f) != screenRotation) setRotation(atoi(f));
      markRx(false);
      break;
    }
    case 'A': {
      char *f = strtok(NULL, "|");
      if (f) setAutoCycle(atoi(f) != 0);
      markRx(false);
      break;
    }
  }
}

// Any valid line from the PC keeps the link alive; `redraw` refreshes the text
void markRx(bool redraw) {
  lastRx = millis();
  if (lastRx == 0) lastRx = 1;
  if (redraw) textDirty = true;
}

// The hardware RX buffer is only 64 bytes, less than two lines from the PC,
// and a redraw can take 100+ ms. Long drawing code calls pumpSerial() to move
// bytes into this bigger ring; readSerial() parses them when the loop is free.
char rxRing[128];
byte rxHead = 0, rxTail = 0;

void pumpSerial() {
  while (Serial.available()) {
    byte next = (rxHead + 1) % sizeof(rxRing);
    if (next == rxTail) return; // ring full: leave the rest in the hardware buffer
    rxRing[rxHead] = Serial.read();
    rxHead = next;
  }
}

// Clear everything inside the gauge ring (plus the status line in its gap)
void clearInner() {
  const int r = GAUGE_R_IN - 1;
  for (int dy = -r; dy <= r; dy++) {
    int w = sqrt((long)r * r - (long)dy * dy);
    tft.drawFastHLine(CX - w, CY + dy, 2 * w + 1, COLOR_BG);
    if ((dy & 7) == 0) pumpSerial();
  }
  tft.fillRect(CX - 60, 198, 120, 13, COLOR_BG);
}

// fillScreen blocks for ~120 ms; clear in strips so serial keeps flowing
void clearScreen() {
  for (int y = 0; y < 240; y += 16) {
    tft.fillRect(0, y, 240, 16, COLOR_BG);
    pumpSerial();
  }
}

void readSerial() {
  pumpSerial();
  while (rxTail != rxHead) {
    char c = rxRing[rxTail];
    rxTail = (rxTail + 1) % sizeof(rxRing);
    if (c == '\r') continue;
    if (c == '\n') {
      rxBuf[rxLen] = '\0';
      if (rxLen == 1) {
        handleCommand(rxBuf[0]);
      } else {
        parseLine();
      }
      rxLen = 0;
    } else if (rxLen < sizeof(rxBuf) - 1) {
      rxBuf[rxLen++] = c;
    } else {
      rxLen = 0;
    }
  }
}

bool isLinked() {
  return lastRx != 0 && millis() - lastRx < LINK_TIMEOUT_MS;
}

void getUptimeStr(char *buf, size_t sz) {
  unsigned long sec = millis() / 1000;
  unsigned int m = (sec / 60) % 60;
  unsigned int h = sec / 3600;
  if (h > 0) snprintf(buf, sz, "%uh %02um", h, m);
  else snprintf(buf, sz, "%um %02lus", m, sec % 60);
}

// -------------------------------------------------------------
// Text helpers
// -------------------------------------------------------------
void printCentered(const char *s, int y, byte size, uint16_t color) {
  pumpSerial();
  int w = strlen(s) * 6 * size;
  tft.setTextSize(size);
  tft.setTextColor(color, COLOR_BG);
  tft.setCursor(CX - w / 2, y);
  // One character at a time: a big-font string takes ~10 ms, long enough
  // for an incoming line to overflow the 64-byte hardware RX buffer
  for (const char *p = s; *p; p++) {
    tft.print(*p);
    pumpSerial();
  }
}

// Fixed labels live in flash (F("...")): every plain string literal also
// takes RAM on the Uno, and RAM is the tighter limit
void printCenteredP(const __FlashStringHelper *s, int y, byte size, uint16_t color) {
  char buf[24];
  strncpy_P(buf, (const char *)s, sizeof(buf) - 1);
  buf[sizeof(buf) - 1] = '\0';
  printCentered(buf, y, size, color);
}

// Text in a band `bandW` px wide. Padded and overwritten in place (no clear),
// so refreshing it never flickers.
void printBand(const char *s, int y, byte size, uint16_t color, int bandW) {
  printPad(s, y, size, color, bandW / (6 * size));
}

// Center `s` inside a fixed `width`-character field padded with spaces.
// Overwrites the old text in one pass, so values updating every second
// don't flicker the way a clear-then-print would.
void printPad(const char *s, int y, byte size, uint16_t color, byte width) {
  char buf[26];
  byte n = min((byte)strlen(s), min(width, (byte)(sizeof(buf) - 1)));
  byte left = (width - n) / 2;
  memset(buf, ' ', width);
  memcpy(buf + left, s, n);
  buf[width] = '\0';
  printCentered(buf, y, size, color);
}

uint16_t levelColor(int pct) {
  if (pct >= 90) return COLOR_ALERT;
  if (pct >= 75) return COLOR_WARN;
  return COLOR_CLAUDE;
}

// -------------------------------------------------------------
// Static UI per Mode
// -------------------------------------------------------------
void drawStaticUI(bool withFrame) {
  if (waiting) {
    drawWaitScreen(withFrame);
    return;
  }
  if (currentMode == MODE_GT) {
    drawGtFace();
    return;
  }
  if (withFrame) {
    tft.drawCircle(CX, CY, 118, COLOR_TRACK);
    drawArc(0, GAUGE_SWEEP * 10, COLOR_TRACK);
    drawTick(GAUGE_START, COLOR_DIM);
    drawTick(GAUGE_START + GAUGE_SWEEP, COLOR_DIM);
    // Ring was redrawn empty
    if (shownPct > 0) drawArc(0, shownPct * GAUGE_SWEEP / 10, ringColor);
  }

  switch (currentMode) {
    case MODE_5H_DIAL:
      printCenteredP(F("CLAUDE"), 36, 2, COLOR_CLAUDE);
      printCenteredP(F("5H WINDOW"), 58, 1, COLOR_DIM);
      printCenteredP(F("TOKENS"), 136, 1, COLOR_DIM);
      tft.drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      break;

    case MODE_TODAY:
      printCenteredP(F("CLAUDE"), 36, 2, COLOR_CLAUDE);
      printCenteredP(F("TODAY USAGE"), 58, 1, COLOR_DIM);
      printCenteredP(F("TOTAL TOKENS"), 136, 1, COLOR_DIM);
      tft.drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      break;

    case MODE_TIMER:
      printCenteredP(F("5H TIMER"), 36, 2, COLOR_CYAN);
      printCenteredP(F("5H RESET COUNTDOWN"), 58, 1, COLOR_DIM);
      printCenteredP(F("TIME REMAINING"), 136, 1, COLOR_DIM);
      tft.drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      break;

    case MODE_DASHBOARD:
      printCenteredP(F("CLAUDE"), 32, 2, COLOR_CLAUDE);
      printCenteredP(F("ALL METRICS"), 50, 1, COLOR_DIM);
      tft.drawFastHLine(CX - 50, 62, 100, COLOR_TRACK);
      tft.drawFastHLine(CX - 50, 172, 100, COLOR_TRACK);
      break;

    case MODE_CLOCK:
      tft.drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      break;

    case MODE_SYSTEM:
      printCenteredP(F("SYSTEM"), 36, 2, sysColor);
      printCenteredP(F("CPU LOAD"), 58, 1, COLOR_DIM);
      tft.drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      break;

    case MODE_CUSTOM:
      tft.drawFastHLine(CX - 40, 150, 80, COLOR_TRACK);
      break;
  }
}

void drawTick(int deg, uint16_t color) {
  float c = cos(deg * DEG2RAD), s = sin(deg * DEG2RAD);
  tft.drawLine(CX + c * GAUGE_R_IN, CY + s * GAUGE_R_IN,
               CX + c * (GAUGE_R_OUT + 3), CY + s * (GAUGE_R_OUT + 3), color);
}

// Radial lines every 0.4 deg, always including the exact end line: 1% is
// 2.7 deg, so stepping by 4 alone would skip the last line and leave a
// colored sliver behind whenever the ring shrinks.
void drawArc(int fromT, int toT, uint16_t color) {
  byte n = 0;
  for (int t = fromT; ; t += 4) {
    if (t > toT) t = toT;
    if (++n % 16 == 0) pumpSerial();
    float rad = (GAUGE_START + t / 10.0) * DEG2RAD;
    float c = cos(rad), s = sin(rad);
    tft.drawLine(CX + c * GAUGE_R_IN, CY + s * GAUGE_R_IN,
                 CX + c * GAUGE_R_OUT, CY + s * GAUGE_R_OUT, color);
    if (t >= toT) break;
  }
}

// -------------------------------------------------------------
// Gauge Animation
// -------------------------------------------------------------
void animateGauge(int target) {
  uint16_t col = getRingColor(target);

  if (col != ringColor && shownPct > 0) {
    drawArc(0, shownPct * GAUGE_SWEEP / 10, col);
  }
  ringColor = col;

  // Clock seconds wrapping 59 -> 0: snap the ring back instead of draining it
  if (currentMode == MODE_CLOCK && target + 50 < shownPct) {
    drawArc(0, shownPct * GAUGE_SWEEP / 10, COLOR_TRACK);
    shownPct = 0;
  }

  if (shownPct == target) return;

  int step = (abs(target - shownPct) > 8) ? 2 : 1;
  int next = shownPct + (target > shownPct ? step : -step);
  if ((step > 1) && ((target > shownPct && next > target) || (target < shownPct && next < target))) {
    next = target;
  }

  int a = shownPct * GAUGE_SWEEP / 10;
  int b = next * GAUGE_SWEEP / 10;
  if (next > shownPct) drawArc(a, b, col);
  else drawArc(b, a, COLOR_TRACK);
  shownPct = next;

  if (currentMode == MODE_5H_DIAL) {
    drawPercent(shownPct);
  }
}

void drawPercent(int pct) {
  char buf[6];
  if (isLinked()) snprintf(buf, sizeof(buf), "%d%%", pct);
  else strcpy(buf, "--");

  byte len = strlen(buf);
  if (len != pctTextLen) {
    tft.fillRect(CX - 50, 78, 100, 32, COLOR_BG);
    pctTextLen = len;
  }
  printCentered(buf, 78, 4, COLOR_TEXT);
}

// -------------------------------------------------------------
// Orbiting comet - runs along the middle of the gauge ring. Its trail is
// erased by repainting the ring's own color underneath, so it never leaves
// stray pixels and never crosses any text.
// -------------------------------------------------------------
int wrapSweep(int a) {
  return ((a % GAUGE_SWEEP) + GAUGE_SWEEP) % GAUGE_SWEEP;
}

// Ring color under sweep position `a` (degrees from GAUGE_START)
uint16_t ringAt(int a) {
  return (shownPct > 0 && a * 10 <= shownPct * GAUGE_SWEEP / 10) ? ringColor : COLOR_TRACK;
}

// A short radial dash in the middle of the ring, the same shape as the
// ring's own lines, so repainting it in the ring color restores it exactly
void orbitDot(int a, uint16_t color) {
  float rad = (GAUGE_START + a) * DEG2RAD;
  float c = cos(rad), s = sin(rad);
  tft.drawLine(CX + c * (ORBIT_R - 3), CY + s * (ORBIT_R - 3),
               CX + c * (ORBIT_R + 3), CY + s * (ORBIT_R + 3), color);
}

void updateOrbit() {
  int step = isLinked() ? 3 : 1;
  int len = ORBIT_TRAIL * 3;

  for (int a = orbitAngle - len; a < orbitAngle - len + step; a++) {
    int w = wrapSweep(a);
    orbitDot(w, ringAt(w));
  }

  orbitAngle = (orbitAngle + step) % GAUGE_SWEEP;
  orbitDot(wrapSweep(orbitAngle - 12), COLOR_DIM);
  orbitDot(wrapSweep(orbitAngle - 1), COLOR_TEXT);
  orbitDot(orbitAngle, COLOR_TEXT);
}

// -------------------------------------------------------------
// Dynamic Readout
// -------------------------------------------------------------
void drawReadout(bool linked) {
  char line[30];

  switch (currentMode) {
    case MODE_5H_DIAL: {
      drawPercent(shownPct);
      printBand(linked ? blockTok : "--", 114, 2, COLOR_CLAUDE, 96);

      if (linked) snprintf(line, sizeof(line), "TODAY %s  %s MSG", todayTok, todayMsgs);
      else strcpy(line, "TODAY --");
      printBand(line, 160, 1, COLOR_TEXT, 140);

      if (!linked) strcpy(line, "RUN BRIDGE ON PC");
      else if (strcmp(timeLeft, "IDLE") == 0) strcpy(line, "BLOCK IDLE");
      else snprintf(line, sizeof(line), "RESET %s", timeLeft);
      printBand(line, 178, 1, COLOR_DIM, 120);
      break;
    }

    case MODE_TODAY: {
      const char *tok = linked ? todayTok : "--";
      static byte lastSz = 0;
      byte sz = (strlen(tok) > 4) ? 3 : 4;
      int y = (sz == 4) ? 78 : 84;
      if (sz != lastSz) { tft.fillRect(CX - 62, 78, 124, 32, COLOR_BG); lastSz = sz; }
      printBand(tok, y, sz, COLOR_TEXT, 120);

      int tPct = todayPct > 0 ? todayPct : calculateTodayPct();
      if (linked) snprintf(line, sizeof(line), "%d%% DAILY PROGRESS", tPct);
      else strcpy(line, "DAILY PROGRESS --");
      printBand(line, 116, 1, COLOR_CYAN, 140);

      if (linked) snprintf(line, sizeof(line), "MESSAGES: %s SENT", todayMsgs);
      else strcpy(line, "MESSAGES: --");
      printBand(line, 160, 1, COLOR_TEXT, 140);

      if (linked) snprintf(line, sizeof(line), "5H BLOCK: %s (%d%%)", blockTok, gaugePct);
      else strcpy(line, "5H BLOCK: --");
      printBand(line, 178, 1, COLOR_DIM, 140);
      break;
    }

    case MODE_TIMER: {
      const char *tStr = linked ? (timeLeft[0] ? timeLeft : "IDLE") : "--";
      static byte lastSz = 0;
      byte sz = (strlen(tStr) > 5) ? 2 : 3;
      int y = (sz == 3) ? 80 : 86;
      if (sz != lastSz) { tft.fillRect(CX - 66, 80, 132, 24, COLOR_BG); lastSz = sz; }
      uint16_t col = (strcmp(tStr, "IDLE") == 0) ? COLOR_WARN : COLOR_CYAN;
      printBand(tStr, y, sz, col, 130);

      int timPct = getTimerPct();
      if (linked && strcmp(tStr, "IDLE") != 0) snprintf(line, sizeof(line), "%d%% TIME REMAINING", timPct);
      else if (linked) strcpy(line, "SESSION IDLE");
      else strcpy(line, "TIMER OFFLINE");
      printBand(line, 116, 1, COLOR_DIM, 140);

      if (linked) snprintf(line, sizeof(line), "5H USAGE: %d%% (%s)", gaugePct, blockTok);
      else strcpy(line, "5H USAGE: --");
      printBand(line, 160, 1, COLOR_TEXT, 140);

      if (linked) snprintf(line, sizeof(line), "TODAY %s  %s MSG", todayTok, todayMsgs);
      else strcpy(line, "TODAY --");
      printBand(line, 178, 1, COLOR_DIM, 140);
      break;
    }

    case MODE_DASHBOARD: {
      char upBuf[12];
      getUptimeStr(upBuf, sizeof(upBuf));

      snprintf(line, sizeof(line), "5H TOK: %s (%d%%)", linked ? blockTok : "--", linked ? gaugePct : 0);
      printBand(line, 68, 1, COLOR_CLAUDE, 140);

      snprintf(line, sizeof(line), "TODAY : %s TOKENS", linked ? todayTok : "--");
      printBand(line, 84, 1, COLOR_TEXT, 140);

      snprintf(line, sizeof(line), "MSGS  : %s SENT", linked ? todayMsgs : "--");
      printBand(line, 100, 1, COLOR_TEXT, 140);

      snprintf(line, sizeof(line), "RESET : %s", linked ? timeLeft : "--");
      printBand(line, 116, 1, COLOR_CYAN, 140);

      snprintf(line, sizeof(line), "LINK  : %s", linked ? "LIVE (115200)" : "OFFLINE");
      printBand(line, 134, 1, linked ? COLOR_LIVE : COLOR_ALERT, 140);

      snprintf(line, sizeof(line), "UPTIME: %s", upBuf);
      printBand(line, 150, 1, COLOR_DIM, 140);
      break;
    }

    case MODE_CLOCK: {
      printPad(linked ? clockDate : "", 36, 2, clockColor, 8);
      printPad(linked ? clockDay : "", 58, 1, COLOR_DIM, 12);
      printPad(linked ? clockTime : "--:--", 78, 4, COLOR_TEXT, 5);
      snprintf(line, sizeof(line), ":%02d", clockSec);
      printPad(linked ? line : "", 114, 2, clockColor, 3);
      printPad(linked ? clockExtra : "RUN APP ON PC", 160, 1, COLOR_DIM, 22);
      break;
    }

    case MODE_SYSTEM: {
      snprintf(line, sizeof(line), "%d%%", cpuPct);
      printPad(linked ? line : "--", 78, 4, COLOR_TEXT, 4);
      snprintf(line, sizeof(line), "RAM %d%%", ramPct);
      printPad(linked ? line : "RAM --", 114, 2, COLOR_PURPLE, 8);
      printPad(linked ? ramText : "", 134, 1, COLOR_DIM, 16);
      printPad(linked ? sysLine1 : "RUN APP ON PC", 160, 1, COLOR_TEXT, 22);
      printPad(linked ? sysLine2 : "", 178, 1, COLOR_DIM, 20);
      break;
    }

    case MODE_CUSTOM: {
      // Pick the biggest text size that fits; clear the box when it changes
      static byte titleSize = 0, valueSize = 0;
      byte ts = strlen(custTitle) <= 8 ? 2 : 1;
      if (ts != titleSize) { tft.fillRect(CX - 48, 36, 96, 16, COLOR_BG); titleSize = ts; }
      printPad(custTitle, ts == 2 ? 36 : 40, ts, custColor, ts == 2 ? 8 : 16);

      printPad(custSub, 58, 1, COLOR_DIM, 18);

      byte vs = strlen(custValue) <= 5 ? 4 : 3;
      if (vs != valueSize) { tft.fillRect(CX - 72, 78, 144, 32, COLOR_BG); valueSize = vs; }
      printPad(custValue, vs == 4 ? 78 : 82, vs, COLOR_TEXT, vs == 4 ? 5 : 8);

      printPad(custLine, 160, 1, COLOR_TEXT, 22);
      break;
    }

    case MODE_GT:
      gtReadout(linked);
      break;
  }

  drawLinkStatus(linked);
}

// -------------------------------------------------------------
// GT gauge: race-car tachometer. Yellow ring with bold black numbers 0-10
// (x10 % CPU), white tick bars outside it, red over the redline, a long
// needle from a silver hub, and a digital readout under the hub.
// -------------------------------------------------------------

// Colour under pixel (x, y) of the face, so the needle can be erased exactly.
// drawGtRing() fills with the same rule.
uint16_t gtBgAt(int x, int y) {
  long d = (long)(x - CX) * (x - CX) + (long)(y - CY) * (y - CY);
  return (d >= (long)GT_BAND_IN * GT_BAND_IN && d <= (long)GT_BAND_OUT * GT_BAND_OUT) ? GT_YELLOW : COLOR_BG;
}

// Largest w with w*w <= n (or < n when `strict`)
int isqrtFloor(long n, bool strict) {
  if (n <= 0) return strict ? -1 : 0;
  int w = sqrt(n);
  while ((long)(w + 1) * (w + 1) <= n) w++;
  while ((long)w * w > n || (strict && (long)w * w >= n)) w--;
  return w;
}

void drawGtRing() {
  for (int dy = -GT_BAND_OUT; dy <= GT_BAND_OUT; dy++) {
    int wo = isqrtFloor((long)GT_BAND_OUT * GT_BAND_OUT - (long)dy * dy, false);
    int wi = isqrtFloor((long)GT_BAND_IN * GT_BAND_IN - (long)dy * dy, true);  // -1: row misses the hole
    tft.drawFastHLine(CX - wo, CY + dy, wo - wi, GT_YELLOW);
    tft.drawFastHLine(CX + wi + 1, CY + dy, wo - wi, GT_YELLOW);
    if ((dy & 7) == 0) pumpSerial();
  }
}

// Radial line at `t` tenths of a degree along the sweep
void gtRay(int t, byte r0, byte r1, uint16_t color) {
  float a = (GAUGE_START + t / 10.0) * DEG2RAD;
  float c = cos(a), s = sin(a);
  tft.drawLine(CX + c * r0, CY + s * r0, CX + c * r1, CY + s * r1, color);
}

// Bold black number on the ring: drawn twice, 1 px apart, with no
// background, so redrawing it over the repaired ring is seamless
void gtNumber(byte i) {
  float a = (GAUGE_START + i * 27) * DEG2RAD;
  int r = (GT_BAND_IN + GT_BAND_OUT) / 2;
  int x = CX + cos(a) * r - (i < 10 ? 5 : 11);
  int y = CY + sin(a) * r - 7;
  tft.setTextSize(2);
  tft.setTextColor(COLOR_BG);
  for (byte b = 0; b < 2; b++) {
    tft.setCursor(x + b, y);
    tft.print(i);
  }
}

void drawGtFace() {
  drawGtRing();
  // Red bar outside the ring over the redline
  for (int t = GT_RED_FROM * 27; t <= GAUGE_SWEEP * 10; t += 4) {
    gtRay(t, GT_BAND_OUT + 2, GT_BAND_OUT + 6, COLOR_ALERT);
    if (t % 40 == 0) pumpSerial();
  }
  // Minor ticks every 2 %, white bars every 10 %
  for (byte p = 0; p <= 100; p += 2) {
    bool red = p >= GT_RED_FROM;
    if (p % 10 == 0) {
      for (char d = -5; d <= 5; d += 5) gtRay(p * 27 + d, GT_BAND_OUT + 2, 118, red ? COLOR_ALERT : COLOR_TEXT);
    } else {
      gtRay(p * 27, 111, 117, red ? COLOR_ALERT : GT_YELLOW);
    }
    pumpSerial();
  }
  for (byte i = 0; i <= 10; i++) gtNumber(i);

  // Silver hub
  tft.fillCircle(CX, CY, 8, 0x8410);
  tft.fillCircle(CX, CY, 5, 0xC618);

  tft.setTextSize(1);
  tft.setTextColor(COLOR_DIM, COLOR_BG);
  tft.setCursor(CX - 15, 170);
  tft.print(F("CPU %"));
  tft.setCursor(CX - 15, 226);
  tft.print(F("x10 %"));
}

// Needle pixel by pixel: 3 px wide near the hub, 1 px at the tip. Erasing
// puts back the face colour of each pixel (ring or black), then the
// numbers it crossed are redrawn.
void gtNeedle(int pct, bool erase) {
  float a = (GAUGE_START + pct * 2.7) * DEG2RAD;
  float c = cos(a), s = sin(a);
  for (byte r = 10; r <= GT_NEEDLE_LEN; r++) {
    char w = r < 70 ? 1 : 0;
    for (char o = -w; o <= w; o++) {
      int x = CX + lround(c * r - s * o);
      int y = CY + lround(s * r + c * o);
      tft.drawPixel(x, y, erase ? gtBgAt(x, y) : COLOR_TEXT);
    }
    if ((r & 15) == 0) pumpSerial();
  }
}

// Move the needle one step toward CPU load (or full scale while sweeping)
void gtUpdate(bool linked) {
  if (gtSweep && gtShown >= 100) gtSweep = false;
  int target = gtSweep ? 100 : (linked ? cpuPct : 0);
  if (gtShown == target) return;

  int next = 0;
  if (gtShown >= 0) {
    int d = target - gtShown;
    int step = gtSweep ? 5 : max(1, abs(d) / 3);
    next = gtShown + constrain(d, -step, step);
    gtNeedle(gtShown, true);
    for (byte i = gtShown / 10; i <= gtShown / 10 + 1 && i <= 10; i++) {
      if (abs(i * 10 - gtShown) <= 5) gtNumber(i);
    }
  }
  gtNeedle(next, false);
  gtShown = next;
}

// Digital readout under the hub: CPU big, then GPU and RAM. Kept inside
// the wedge the needle never sweeps, so it never needs repairing.
void gtReadout(bool linked) {
  static byte lastLen = 0;
  char v[8];
  if (linked) itoa(cpuPct, v, 10);
  else strcpy(v, "--");
  byte len = strlen(v);
  if (len != lastLen) {
    tft.fillRect(CX - 27, 146, 54, 22, COLOR_BG);
    lastLen = len;
  }
  tft.setTextSize(3);
  tft.setTextColor(linked && cpuPct >= GT_RED_FROM ? COLOR_ALERT : COLOR_TEXT, COLOR_BG);
  tft.setCursor(CX - len * 9 + 1, 146);
  tft.print(v);

  // "GPU 42  RAM 48" in fixed-width fields, overwritten in place
  tft.setTextSize(1);
  tft.setCursor(CX - 42, 180);
  for (byte k = 0; k < 2; k++) {
    byte p = k ? ramPct : gpuPct;
    tft.setTextColor(COLOR_DIM, COLOR_BG);
    tft.print(k ? F("  RAM") : F("GPU"));
    if (linked && p <= 100) snprintf(v, sizeof(v), "%3d", p);
    else strcpy(v, " --");
    tft.setTextColor(linked && p <= 100 && p >= GT_RED_FROM ? COLOR_ALERT : COLOR_TEXT, COLOR_BG);
    tft.print(v);
  }
}

// -------------------------------------------------------------
// Waiting screen: shown while no PC is sending. A violet spinner chases
// round the full ring and three dots pulse. When data arrives the widget
// that was showing comes back.
// -------------------------------------------------------------
#define WAIT_TAIL 80  // spinner length, degrees
#define WAIT_STEP 6   // degrees per frame

void setWaiting(bool on) {
  waiting = on;
  clearScreen();
  shownPct = 0;
  pctTextLen = 0;
  gtShown = -1;
  gtSweep = currentMode == MODE_GT;
  drawStaticUI();
  textDirty = true;
}

// Radial lines across the ring every 0.5 deg from `from` to `to` (degrees,
// any range). Painting and erasing use the same lines, so nothing is left.
void waitArc(int from, int to, uint16_t color) {
  for (int t = from * 2; t <= to * 2; t++) {
    float a = t * 0.5 * DEG2RAD;
    float c = cos(a), s = sin(a);
    tft.drawLine(CX + c * GAUGE_R_IN, CY + s * GAUGE_R_IN, CX + c * GAUGE_R_OUT, CY + s * GAUGE_R_OUT, color);
    if ((t & 15) == 0) pumpSerial();
  }
}

void drawWaitScreen(bool withFrame) {
  if (withFrame) {
    tft.drawCircle(CX, CY, 118, COLOR_TRACK);
    waitArc(0, 360, COLOR_TRACK);
    waitArc(waitHead - WAIT_TAIL, waitHead - WAIT_STEP, COLOR_BRAND);
    waitArc(waitHead - WAIT_STEP + 1, waitHead, COLOR_TEXT);
  }
  printCenteredP(F("ORBDASH"), 70, 3, COLOR_BRAND);
  printCenteredP(F("WAITING FOR PC"), 108, 1, COLOR_TEXT);
  printCenteredP(F("START THE ORBDASH APP"), 156, 1, COLOR_DIM);
  printCenteredP(F("USB  115200"), 170, 1, COLOR_DIM);
}

void waitAnimation() {
  // Coral body with a white head: the tail end goes back to the track, the
  // old head turns coral, and a new head is drawn in front of it
  waitArc(waitHead - WAIT_TAIL, waitHead - WAIT_TAIL + WAIT_STEP - 1, COLOR_TRACK);
  waitArc(waitHead - WAIT_STEP + 1, waitHead, COLOR_BRAND);
  waitArc(waitHead + 1, waitHead + WAIT_STEP, COLOR_TEXT);
  waitHead = (waitHead + WAIT_STEP) % 360;

  // Three dots, lit one after another
  for (byte i = 0; i < 3; i++) {
    tft.fillCircle(CX - 14 + i * 14, 132, 3, (frameCount / 8) % 4 == i ? COLOR_EMBER : COLOR_TRACK);
  }
}

void drawLinkStatus(bool linked) {
  if (waiting) return;
  if (currentMode == MODE_GT) return;  // its line would sit on the yellow ring
  const int y = 200;

  bool on = linked || (frameCount / 15) % 2 == 0;
  uint16_t dot = linked ? COLOR_LIVE : COLOR_ALERT;
  const char *label = linked ? "LIVE" : "OFF";

  const char *modeNames[] = {"5H", "TODAY", "TIMER", "ALL", "CLOCK", "SYS", "CUSTOM", "GT"};
  char tag[22];
  if (autoCycle) {
    snprintf(tag, sizeof(tag), "%s [AUTO %s]", label, modeNames[currentMode]);
  } else {
    snprintf(tag, sizeof(tag), "%s [%d/%d:%s]", label, currentMode + 1, NUM_MODES, modeNames[currentMode]);
  }

  // Fixed 18-char field: overwriting it in place means no clear, no flicker
  byte n = strlen(tag);
  while (n < 18) tag[n++] = ' ';
  tag[18] = '\0';
  const int x = CX - (10 + 18 * 6) / 2;
  tft.fillCircle(x + 3, y + 4, 3, on ? dot : COLOR_BG);
  tft.setTextSize(1);
  tft.setTextColor(COLOR_DIM, COLOR_BG);
  tft.setCursor(x + 10, y + 1);
  tft.print(tag);
}
