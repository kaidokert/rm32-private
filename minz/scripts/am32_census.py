#!/usr/bin/env python3
"""AM32 spike census: throttle ladder + SPK-line/KISS dual logger.

Companion to the SPIKE_STATS instrumentation added to AM32 main.c
(2026-07-16): AM32 emits an ASCII "SPK n=X ms=X dep=X base=X" line
every 2 s on the telemetry UART, interleaved with the 10-byte KISS
frames BF requests. This script drives the ladder and separates the
two streams: KISS frames CRC-gate; SPK lines regex-gate.

Census definition parity with minz: events = clusters of 1 ms windows
whose 20 kHz-sampled current max exceeds baseline + 56 raw counts
(+1.5 A); dep = worst depth above baseline in raw counts (37.2/A).

Usage:
  python scripts/am32_census.py --rungs 30,40,45,50 --dwell 20
"""

import argparse
import pathlib
import re
import statistics as st
import sys
import time

import serial

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_ref import Throttle, parse_kiss  # noqa: E402

SPK_RE = re.compile(rb"SPK n=(\d+) ms=(\d+) dep=(\d+) b=(\d+)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf-port", default="COM42")
    ap.add_argument("--tlm-port", default="COM41")
    ap.add_argument("--rungs", default="30,40,45,50")
    ap.add_argument("--dwell", type=float, default=20.0)
    ap.add_argument("--max-amps", type=float, default=5.0)
    ap.add_argument("--tag", default="am32census")
    a = ap.parse_args()
    rungs = [int(x) for x in a.rungs.split(",")]
    capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
    stamp = time.strftime("%H%M%S")
    csv = open(capdir / f"{a.tag}_{stamp}.csv", "w")
    csv.write(
        "throttle_pct,f_e_hz,amps,volts,spk_events,spk_ms,spk_dep_raw,"
        "spk_base_raw,spk_lines,secs\n"
    )
    thr = Throttle(a.bf_port)
    thr.start()
    tlm = serial.Serial(a.tlm_port, 115200, timeout=0.05)
    try:
        print("idle hold 5s (clean arm)...", flush=True)
        thr.value = 1000
        time.sleep(5.0)
        last_erpm = None
        for pct in rungs:
            thr.value = 1000 + 10 * pct
            time.sleep(2.0)
            tlm.reset_input_buffer()
            buf = bytearray()
            end = time.monotonic() + a.dwell
            dead = False
            while time.monotonic() < end:
                buf += tlm.read(4096)
                r = parse_kiss(bytes(buf[-40:]))
                if r:
                    if r[-1][2] > a.max_amps:
                        print(f"  !! {r[-1][2]:.2f} A - abort", flush=True)
                        dead = True
                        break
                    if last_erpm and r[-1][4] < last_erpm * 0.5:
                        print(
                            f"  !! eRPM collapse (REBOOT) - abort", flush=True
                        )
                        dead = True
                        break
            if dead:
                thr.value = 1000
                break
            frames = parse_kiss(bytes(buf))
            spk = SPK_RE.findall(bytes(buf))
            if frames:
                amps = st.median(f[2] for f in frames)
                volts = st.median(f[1] for f in frames)
                erpm = st.median(f[4] for f in frames)
                last_erpm = erpm
                fe = erpm / 60.0
            else:
                amps = volts = fe = 0.0
            ev = sum(int(m[0]) for m in spk)
            ems = sum(int(m[1]) for m in spk)
            dep = max((int(m[2]) for m in spk), default=0)
            base = st.median([int(m[3]) for m in spk]) if spk else 0
            secs = 2.0 * len(spk)
            rate = ev / secs if secs else float("nan")
            print(
                f"  thr {pct:3d}%: f_e={fe:6.0f} Hz I={amps:5.2f} A "
                f"V={volts:5.2f} | spikes={ev} ({rate:4.2f}/s) "
                f"over-ms={ems} depth={dep} raw (~{dep/37.2:.1f} A) "
                f"base={base:.0f} [{len(spk)} SPK lines]",
                flush=True,
            )
            csv.write(
                f"{pct},{fe:.0f},{amps:.2f},{volts:.2f},{ev},{ems},{dep},"
                f"{base:.0f},{len(spk)},{secs:.0f}\n"
            )
    finally:
        print("throttle down + release", flush=True)
        thr.stop()
        csv.close()
    print(f"csv: captures/{a.tag}_{stamp}.csv")


if __name__ == "__main__":
    main()
