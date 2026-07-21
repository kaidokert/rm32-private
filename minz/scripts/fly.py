#!/usr/bin/env python3
"""Interactive throttle terminal for am32_clone (operator flight
stick). Host-side only: maps single keys onto the UART_DUTY_MODE
protocol and owns the keepalives (the firmware's 3 s deadman cuts
throttle if the host dies — that is the safety net, keep it).

Keys:
  1..9   throttle 10..90 %
  0 / SPACE  cut (0 %)
  f      100 %
  + / -  +-5 %
  i      status line (f_e, mA, mV)
  b      black-box dump
  q / ESC    kill and quit

The full wire (ZC trace included) records to captures/fly_<tag>_raw.bin
and decodes to CSV on exit; the tail report runs automatically — your
manual session IS a test capture.

Usage: python scripts/fly.py [--tag fly1]
"""

import argparse
import pathlib
import re
import sys
import time

import msvcrt
import serial

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write  # noqa: E402
import zctrace_capture as z  # noqa: E402

PORT = "COM41"
BAUD = 2_000_000
MA_PER_RAW = 3300.0 / 4095.0 / 30.0 * 1000.0
MV_PER_RAW = 3300.0 / 4095.0 * 9.33

ap = argparse.ArgumentParser()
ap.add_argument("--tag", default="fly")
a = ap.parse_args()

ser = serial.Serial(PORT, BAUD, timeout=0.02)
cap = bytearray()
pct = 0
last_ka = 0.0
last_status = 0.0


def send_pct(p):
    paced_write(ser, f"{p}\n".encode())


def pump(secs=0.0):
    t0 = time.monotonic()
    while True:
        b = ser.read(8192)
        if b:
            cap.extend(b)
        if time.monotonic() - t0 >= secs:
            return


def status():
    ser.read(200000)
    paced_write(ser, b"i")
    t0 = time.monotonic()
    buf = b""
    while time.monotonic() - t0 < 0.5:
        b = ser.read(65536)
        buf += b
        cap.extend(b)
    m = None
    for m in re.finditer(
            rb"i step=\d+ old=(\d+) run=(\d+) ci=\d+ avg=(\d+) "
            rb"zc=\d+ duty=(\d+) iraw=(\d+) vbat=(\d+)", buf):
        pass
    if m:
        old, run, avg, duty, iraw, vbat = (int(m.group(k))
                                           for k in range(1, 7))
        fe = 2e6 / (6 * avg) if avg else 0.0
        print(f"\r  [{pct:3d}%] {'RUN' if run else 'off'}"
              f"{' poll' if old else ''}  {fe:6.0f} Hz  "
              f"{iraw * MA_PER_RAW:6.0f} mA  "
              f"{vbat * MV_PER_RAW:5.0f} mV  duty {duty:4d}   ",
              end="", flush=True)
    else:
        print(f"\r  [{pct:3d}%] (no readback)                    ",
              end="", flush=True)


print(__doc__.split("Keys:")[1].split("The full")[0])
print("armed; keys live. SPACE/0 cuts, q quits.\n")
send_pct(0)
pump(1.0)

try:
    while True:
        if msvcrt.kbhit():
            ch = msvcrt.getch()
            if ch in (b"q", b"\x1b"):
                break
            elif ch in (b"0", b" "):
                pct = 0
                send_pct(0)
                print(f"\r  [  0%] CUT                            ",
                      end="", flush=True)
            elif ch.isdigit():
                pct = int(ch) * 10
                send_pct(pct)
            elif ch == b"f":
                pct = 100
                send_pct(pct)
            elif ch == b"+":
                pct = min(100, pct + 5)
                send_pct(pct)
            elif ch == b"-":
                pct = max(0, pct - 5)
                send_pct(pct)
            elif ch == b"i":
                status()
            elif ch == b"b":
                paced_write(ser, b"b")
                pump(0.8)
                txt = bytes(b for b in cap[-4000:]
                            if 32 <= b < 127 or b == 10)
                print("\n" + txt.decode("ascii", "replace"))
        now = time.monotonic()
        if now - last_ka > 0.8:
            send_pct(pct)
            last_ka = now
        if now - last_status > 2.0:
            status()
            last_status = now
        pump(0.05)
finally:
    for _ in range(2):
        paced_write(ser, b"0\n")
        time.sleep(0.4)
    paced_write(ser, b"w")
    pump(0.5)
    ser.close()

raw_p = pathlib.Path("captures") / f"fly_{a.tag}_raw.bin"
raw_p.write_bytes(bytes(cap))
recs = z.decode(bytes(cap))
csv_p = pathlib.Path("captures") / f"fly_{a.tag}_trace.csv"
with open(csv_p, "w") as f:
    f.write("step,old,zt_us,ci_us,wait_us,duty,tenkhz,avg_us\n")
    for r in recs:
        f.write(f"{r['step']},{int(r['old'])},{r['zt_ticks'] / 2:.1f},"
                f"{r['ci_ticks'] / 2:.1f},{r['wait_ticks'] / 2:.1f},"
                f"{r['duty']},{r['tenkhz']},{r['avg_ticks'] / 2:.1f}\n")
print(f"\nsession: {len(cap)} B, {len(recs)} trace records")
print(f"raw: {raw_p}  csv: {csv_p}")
print("tail report:")
import subprocess

subprocess.run([sys.executable, "scripts/zct_sweep_report.py",
                str(csv_p)])
