#!/usr/bin/env python3
"""Operating-map sweep: throttle rungs vs f_e / amps / volts, for the
am32_clone (--mode clone) or the AM32 NOTRACE build (--mode am32).
Both are driven over the same UART_DUTY_MODE protocol; the readback
differs:

  clone: per-rung `i` poll -> "i ... avg=<ticks> ... iraw=N vbat=N"
         f_e = 2e6/(6*avg_ticks); mA/mV via the minz sense calibration.
  am32:  KISS telemetry frames (NOTRACE build = clean wire) ->
         volts, amps, eRPM (f_e = erpm/60; cross-check against the
         zctsweep trace-derived f_e before trusting absolute rpm).

Writes captures/map_<tag>.csv: rung,f_e_hz,ma,mv
Usage:
    python scripts/map_sweep.py --mode clone --tag clone_map
    python scripts/map_sweep.py --mode am32  --tag am32_map
"""

import argparse
import pathlib
import re
import sys
import time

import serial

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write, bootloader_spray  # noqa: E402
from am32_ref import parse_kiss  # noqa: E402

PORT = "COM41"
BAUD = 2_000_000

# minz sense calibration (core/sense.rs): 0.806 mV/count VREF-nominal,
# ISNS 30 mV/A, VBAT divider 9.33.
MA_PER_RAW = 3300.0 / 4095.0 / 30.0 * 1000.0  # ~26.9 mA/count
MV_PER_RAW = 3300.0 / 4095.0 * 9.33  # ~7.52 mV/count

ap = argparse.ArgumentParser()
ap.add_argument("--mode", choices=["clone", "am32"], required=True)
ap.add_argument("--tag", required=True)
ap.add_argument("--rungs", default="10,20,30,40,50,60,70,80,90,100")
ap.add_argument("--dwell", type=float, default=6.0)
ap.add_argument("--spray", action="store_true",
                help="bootloader garbage spray first (am32 with BL)")
a = ap.parse_args()
rungs = [int(r) for r in a.rungs.split(",")]

ser = serial.Serial(PORT, BAUD, timeout=0.05)
cap = bytearray()
rows = []


def hold(pct, secs):
    t0 = time.monotonic()
    last = 0.0
    buf = bytearray()
    while time.monotonic() - t0 < secs:
        if time.monotonic() - last > 0.8:
            paced_write(ser, f"{pct}\n".encode())
            last = time.monotonic()
        b = ser.read(8192)
        buf += b
        cap.extend(b)
    return bytes(buf)


def read_clone_info(pct):
    # drain, poke `i`, parse the freshest line
    ser.read(65536)
    for _ in range(5):
        paced_write(ser, b"i")
        time.sleep(0.4)
        buf = ser.read(65536)
        cap.extend(buf)
        m = None
        for m in re.finditer(
                rb"i step=\d+ old=\d+ run=(\d+) ci=\d+ avg=(\d+) "
                rb"zc=\d+ duty=(\d+) iraw=(\d+) vbat=(\d+)", buf):
            pass
        if m:
            run, avg, duty, iraw, vbat = (int(m.group(k)) for k in
                                          range(1, 6))
            if run and avg:
                fe = 2e6 / (6 * avg)
                return fe, iraw * MA_PER_RAW, vbat * MV_PER_RAW, duty
    return None


def read_am32_kiss(tail):
    fr = [f for f in parse_kiss(tail[-30000:]) if 3.0 < f[1] < 13.0
          and 0.0 <= f[2] < 40.0]
    if not fr:
        return None
    # median of the last few frames (garble-resistant)
    fr = fr[-9:]
    volts = sorted(f[1] for f in fr)[len(fr) // 2]
    amps = sorted(f[2] for f in fr)[len(fr) // 2]
    erpm = sorted(f[4] for f in fr)[len(fr) // 2]
    return erpm / 60.0, amps * 1000.0, volts * 1000.0, 0


try:
    if a.spray:
        bootloader_spray(ser)
        time.sleep(0.5)
    print("idle 5s (clean arm)...")
    hold(0, 5.0)
    started = False
    for att in range(4):
        hold(rungs[0], 4.0)
        if a.mode == "clone":
            r = read_clone_info(rungs[0])
            # require a real spin-up, not a dead-start plateau (rm32's
            # engage lottery can hand back run=1 at noise-pace f_e)
            started = r is not None and r[0] > 300
        else:
            r = read_am32_kiss(bytes(cap))
            started = r is not None and r[0] > 300
        if started:
            print(f"started at {rungs[0]}%")
            break
        paced_write(ser, b"0\n")
        time.sleep(2.0)
    if not started:
        raise SystemExit("no start")
    for pct in rungs:
        tail = hold(pct, a.dwell)
        if a.mode == "clone":
            r = read_clone_info(pct)
        else:
            r = read_am32_kiss(tail)
        if r:
            fe, ma, mv, duty = r
            rows.append((pct, fe, ma, mv, duty))
            print(f"{pct:3d}%: f_e {fe:6.0f} Hz  {ma:6.0f} mA  "
                  f"{mv:5.0f} mV  duty {duty}")
        else:
            print(f"{pct:3d}%: NO READBACK")
finally:
    paced_write(ser, b"0\n")
    time.sleep(1.0)
    paced_write(ser, b"0\n")
    ser.close()

out = pathlib.Path("captures") / f"map_{a.tag}.csv"
with open(out, "w") as f:
    f.write("rung,f_e_hz,ma,mv,duty\n")
    for pct, fe, ma, mv, duty in rows:
        f.write(f"{pct},{fe:.0f},{ma:.0f},{mv:.0f},{duty}\n")
print(f"wrote {out} ({len(rows)} rungs)")
