"""
Claude usage -> RoundTFT bridge.

The Arduino Uno/Nano runs the RoundTFT display. This script runs on the PC,
reads local Claude Code session logs, and streams real-time usage metrics
to the Arduino over serial.

Usage:
    python claude_usage_bridge.py               # auto-detects Arduino port
    python claude_usage_bridge.py --port COM5   # specify serial port
    python claude_usage_bridge.py --dry-run     # test/print metrics without serial
"""

import argparse
import json
import os
import sys
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

BLOCK = timedelta(hours=5)
LOG_ROOT = Path.home() / ".claude" / "projects"


def human(n):
    for unit, size in (("B", 1e9), ("M", 1e6), ("K", 1e3)):
        if n >= size:
            v = n / size
            return f"{v:.1f}{unit}" if v < 100 else f"{v:.0f}{unit}"
    return str(int(n))


def parse_limit(s):
    s = s.strip().upper()
    mult = {"K": 1e3, "M": 1e6, "B": 1e9}.get(s[-1:], 1)
    return int(float(s.rstrip("KMB")) * mult)


# Per-file cache so we only re-parse files that changed.
_cache = {}


def read_entries():
    entries = {}
    for f in LOG_ROOT.rglob("*.jsonl"):
        try:
            mtime = f.stat().st_mtime
        except OSError:
            continue
        cached = _cache.get(f)
        if not cached or cached[0] != mtime:
            rows = []
            try:
                with f.open(encoding="utf-8", errors="ignore") as fh:
                    for line in fh:
                        if '"usage"' not in line:
                            continue
                        try:
                            d = json.loads(line)
                        except ValueError:
                            continue
                        msg = d.get("message") or {}
                        u = msg.get("usage")
                        ts = d.get("timestamp")
                        if d.get("type") != "assistant" or not u or not ts:
                            continue
                        tokens = (u.get("input_tokens", 0) + u.get("output_tokens", 0)
                                  + u.get("cache_creation_input_tokens", 0)
                                  + u.get("cache_read_input_tokens", 0))
                        key = (msg.get("id"), d.get("requestId"))
                        t = datetime.fromisoformat(ts.replace("Z", "+00:00"))
                        rows.append((key, t, tokens))
            except OSError:
                continue
            cached = _cache[f] = (mtime, rows)
        for key, t, tokens in cached[1]:
            entries[key] = (t, tokens)
    return sorted(entries.values())


def summarize(entries, limit):
    now = datetime.now(timezone.utc)

    start = last = None
    block_tokens = 0
    for t, tok in entries:
        if start is None or t - start >= BLOCK or t - last >= BLOCK:
            start = t.replace(minute=0, second=0, microsecond=0)
            block_tokens = 0
        block_tokens += tok
        last = t

    active = start is not None and now - start < BLOCK and now - last < BLOCK
    if active:
        left = start + BLOCK - now
        mins = int(left.total_seconds() // 60)
        left_s = f"{mins // 60}h{mins % 60:02d}m"
        if limit:
            pct = min(100, int(block_tokens * 100 / limit))
        else:
            pct = int((now - start) / BLOCK * 100)
    else:
        block_tokens, pct, left_s = 0, 0, "IDLE"

    local_midnight = datetime.now().astimezone().replace(
        hour=0, minute=0, second=0, microsecond=0)
    today = [tok for t, tok in entries if t >= local_midnight]
    today_sum = sum(today)
    today_goal = (limit * 2) if limit else 2_000_000
    today_pct = min(100, int(today_sum * 100 / today_goal)) if today_goal > 0 else 0

    return f"U|{human(block_tokens)}|{pct}|{human(today_sum)}|{len(today)}|{left_s}|{today_pct}"


def find_port():
    from serial.tools import list_ports
    ports = list(list_ports.comports())
    for p in ports:
        desc = f"{p.description} {p.manufacturer or ''}".lower()
        if any(k in desc for k in ("arduino", "ch340", "ch341", "usb serial", "usb-serial")):
            return p.device
    return ports[0].device if len(ports) == 1 else None


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", help="serial port, e.g. COM5 (default: auto-detect)")
    ap.add_argument("--baud", type=int, default=115200)
    ap.add_argument("--interval", type=float, default=5, help="seconds between updates")
    ap.add_argument("--block-limit", help="token budget per 5h block, e.g. 50M")
    ap.add_argument("--dry-run", action="store_true", help="print lines instead of sending")
    args = ap.parse_args()

    limit = parse_limit(args.block_limit) if args.block_limit else 0

    ser = None
    if not args.dry_run:
        import serial
        port = args.port or find_port()
        if not port:
            sys.exit("No Arduino found. Plug it in or pass --port COMx (or use --dry-run).")
        ser = serial.Serial(port, args.baud, timeout=1)
        print(f"Connected to {port}; waiting for the board to reset...")
        time.sleep(2.5)

    print("Claude usage bridge running. Streaming metrics to RoundTFT...")
    while True:
        line = summarize(read_entries(), limit)
        print(time.strftime("%H:%M:%S"), line)
        if ser:
            ser.write((line + "\n").encode("ascii"))
        if args.dry_run and args.interval <= 0:
            break
        time.sleep(args.interval)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
