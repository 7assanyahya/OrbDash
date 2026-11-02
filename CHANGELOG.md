# Changelog

## 1.2.0 (2026-11-02)

### New

- **OrbDash Mini:** an ESP32-C3 SuperMini with one display in a small pod
  (about 50 × 52 × 56 mm). Case in `case/mini/`, wiring in the README.
- **OrbDash Pro:** an ESP32-S3 DevKit with three displays in a row on a wide
  console (about 142 × 86 × 58 mm). Case in `case/pro/`.
- **ESP32 firmware** (`firmware/OrbDashESP`) for both: the same eight
  widgets, the boot and waiting animations, the button and the serial
  protocol. Frames are drawn in memory and sent whole, so nothing flickers.
  The board you compile for picks the device.
- **Three screens in the app:** on the Pro, a Left / Middle / Right picker on
  the Dashboard and Widgets pages chooses which screen a click goes to. The
  sidebar names the device (OrbDash, Mini or Pro) and its firmware.
- **Protocol:** `W|<mode>|<screen>`, `MODE|<n>|<screen>`, the `?` request,
  and `ORB|READY` now also reports the screen count and device.
- Wiring diagrams for both new devices (`docs/wiring_esp.py`).

### Fixed

- **Port detection by USB ID:** the app used to treat any "USB Serial
  Device" as a board, which includes 3D printers. Auto-connect now only picks
  Arduino, CH340, CP210x, FTDI and Espressif boards, and never guesses.
- The app builds again: `Cargo.toml` had an `edition` that doesn't exist.

The Uno firmware is unchanged (1.1.0) and works with this app.

## 1.1.0 (2026-11-01)

### New

- **GT Gauge widget** (widget 8): a race-car style tachometer. A yellow ring
  with a 0–10 scale, red redline from 85 %, a needle for CPU load that sweeps
  to full scale when the widget opens, and a readout with GPU and RAM.
- **GPU load** in the app, from `nvidia-smi` on NVIDIA cards or Windows' GPU
  engine counters on others. Shown on the GT gauge and the System page.
- **Waiting animation:** when no data arrives for 15 seconds, and at power-up
  until the app connects, the screen shows a spinner and "Waiting for PC".
  It goes back to the widget as soon as data comes in.
- **3D printer pages:** templates for OctoPrint, Klipper (Moonraker) and
  UltiMaker printers. Progress on the ring, time left below.
- **New app design:** dark, card-based, with a sidebar of pages (Dashboard,
  Widgets, Custom pages, Claude, System, Clock, Settings). The dashboard has a
  live preview, stat tiles and one-click widget cards.
- **Brand:** logo, app icon, colours (Orb Violet and Ember), type and a brand
  guide. The app has its own window icon, and the board's boot and waiting
  screens use the brand colours.
- **Demo mode:** run the app with `ORBDASH_DEMO=1` to show sample Claude
  numbers instead of your own, for screenshots.
- **Versions:** the app shows its version, and the board reports its firmware
  version at startup (`ORB|READY|8|1.1.0`), shown next to the port.

### Fixed

- **Case:** the display pocket now has a slot for the board's header tab and
  room for its solder joints, so the screen sits flat. The display is 4 mm
  higher so the header plugs clear the Uno. New settings: `tab_w`,
  `tab_reach`, `joint_h`. Reprint the shell and the retainer; the base plate
  is unchanged.
- The firmware's fixed screen labels moved to flash, freeing RAM (75 % → 70 %).
- Network speeds in the app show their unit (B/s).
- Settings saved by 1.0 load fine, including the widget rotation list.

### Upgrading

Flash the new firmware (`firmware/OrbDash`) and run the new app together:
the GT gauge needs both. The 1.1.0 app also works with 1.0 firmware, without
the GT gauge and the firmware version.

## 1.0.0

First release: firmware with seven widgets (Claude 5H, Today, Timer, All,
Clock, System, Custom), the Rust desktop app with custom pages, the wiring
diagram and the 3D-printable case.
