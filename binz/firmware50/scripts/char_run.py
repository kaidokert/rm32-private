#!/usr/bin/env python3
"""Battery characterization runner (study 2026-10-01): one profile, one capture.

Drives the `char-capture` measurement image (`bin/char-capture.rs`): installs a
profile with `{period settle post d,s,h ...}`, checks the firmware's `PROFILE`
echo against what was sent, then sends `P` and saves everything through
`CHAREND` to `captures/char/<label>.txt`. Every run in the study goes through
this one script, so every capture has the same wire format and the same parser
(`scripts/char_parse.py`).

Waypoints are `duty,slew,hold`: duty in tenths, slew in tenths per second (0 = a
step, 65535 = the production staircase, 1 % per 500 ms), hold in ms. `period`
is ADC scans (101 µs) per recorder sample; `settle` the ms after the first
waypoint is reached before the commutation histograms count; `post` the ms the
recorder keeps running after the last waypoint starts (0 = to the end).

Usage:
    python scripts/char_run.py --elf captures/elf/<crc>.char-v1.elf --flash \\
        --label map-500 --period 500 --settle 3000 --post 0 --wp 500,65535,12000
"""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
import time

import serial  # type: ignore

import bemf_run

REPO = pathlib.Path(__file__).resolve().parent.parent
PROBE = "0483:374b:066CFF343433464757233430"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default=bemf_run.DEFAULT_PORT)
    ap.add_argument("--elf", required=True)
    ap.add_argument("--flash", action="store_true")
    ap.add_argument("--label", required=True)
    ap.add_argument("--period", type=int, required=True)
    ap.add_argument("--settle", type=int, default=0)
    ap.add_argument("--post", type=int, default=0)
    ap.add_argument("--wp", nargs="+", required=True, help="duty,slew,hold ...")
    ap.add_argument("--timeout", type=float, default=140.0)
    args = ap.parse_args()
    out = REPO / "captures" / "char" / f"{args.label}.txt"
    if out.exists():
        sys.exit(f"REFUSED: {out} exists; choose another --label")
    out.parent.mkdir(parents=True, exist_ok=True)
    bemf_run.ELF = pathlib.Path(args.elf)
    if args.flash:
        for cmd in (["probe-rs", "download", "--chip", "STM32G071RBTx", "--probe", PROBE, args.elf],
                    ["probe-rs", "reset", "--chip", "STM32G071RBTx", "--probe", PROBE]):
            if subprocess.run(cmd, check=False).returncode:
                print("flash failed; refusing to run")
                return 2
        time.sleep(1.5)
    spec = f"{{{args.period} {args.settle} {args.post} {' '.join(args.wp)}}}"
    with serial.Serial(args.port, 115200, timeout=0.2) as port:
        port.reset_input_buffer()
        port.write(spec.encode())
        port.flush()
        echo = b""
        t0 = time.time()
        while time.time() - t0 < 3.0:
            echo += port.read(256)
            tail = echo.split(b"PROFILE", 1)
            if len(tail) == 2 and b"\n" in tail[1]:
                break
        line = next((l for l in echo.decode(errors="replace").splitlines() if l.startswith("PROFILE")), "")
        print(f"sent {spec}\n  -> {line}")
        want = [tuple(int(x) for x in w.split(",")) for w in args.wp]
        got = [tuple(int(x) for x in m.groups()) for m in re.finditer(r"wp=(\d+),(\d+),(\d+)", line)]
        if got != want or f"period_scans={args.period}" not in line:
            print("REFUSED: the firmware's PROFILE echo does not match what was sent")
            port.write(bemf_run.STOP)
            return 2
        try:
            ok = bemf_run.capture_one(port, out, args.timeout, b"P", end_marker="CHAREND")
        finally:
            port.write(bemf_run.STOP)
            port.flush()
    if not ok:
        print("capture did not complete (no CHAREND)")
        return 1
    with out.open("a", encoding="utf-8") as fh:
        fh.write(f"# profile {spec}\n")
    text = out.read_text(encoding="utf-8")
    done = re.search(r"BEMFDONE reason=(\d+)", text)
    recs = sum(1 for l in text.splitlines() if l.startswith("CHARREC "))
    print(f"saved {out.name}: reason={done.group(1) if done else '?'} rec={recs}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
