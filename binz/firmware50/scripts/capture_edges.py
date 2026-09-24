#!/usr/bin/env python3
"""Record the 25% loop's COMP decisions for the replay test (goal item 7).

Flash the diagnostic `edge-capture` image first (it is `shell-pwm` with the
COMP root's decisions recorded; see `bin/edge-capture.rs`). This runs the
same unjudged 15% warm-up the step check uses, then the 25% rung, and saves
everything through `CAPEND` -- the run's report and the capture -- to
`captures/replay/<label>.txt`, with the image's SHA-256 in the header.

The capture's run is then judged against the cohort (`cohort.check`): a
capture of a loop outside the cohort does not pin the cohort's sequence.

Usage:
    python scripts/capture_edges.py --label e121-capture25
"""

from __future__ import annotations

import argparse
import pathlib
import sys
import time

import serial  # type: ignore

import bemf_run
import cohort

REPO = pathlib.Path(__file__).resolve().parent.parent
ELF = REPO / "target" / "thumbv6m-none-eabi" / "release" / "edge-capture"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default=bemf_run.DEFAULT_PORT)
    ap.add_argument("--label", required=True)
    ap.add_argument("--timeout", type=float, default=110.0)
    # `5` captures the first window at 25% (the replay capture, E121); `c`
    # captures the LAST decisions at 30%, which is what a run that stops on a
    # fault needs (E138). The firmware arms the ring circularly for `c`.
    ap.add_argument("--command", default="5", choices=["5", "c"])
    args = ap.parse_args()
    bemf_run.ELF = ELF
    out_dir = REPO / "captures" / "replay"
    out_dir.mkdir(parents=True, exist_ok=True)
    warm = out_dir / f"{args.label}-warmup.txt"
    out = out_dir / f"{args.label}.txt"
    with serial.Serial(args.port, 115200, timeout=0.2) as port:
        try:
            print("== warm-up 15% (unjudged)")
            if not bemf_run.capture_one(port, warm, 85.0, b"b"):
                print("warm-up did not complete")
                return 1
            time.sleep(20.0)
            print(f"== key {args.command} with capture -> {out}")
            if not bemf_run.capture_one(port, out, args.timeout, args.command.encode(), end_marker="CAPEND"):
                print("capture did not complete (no CAPEND)")
                return 1
        finally:
            port.write(bemf_run.STOP)
            port.flush()
    text = out.read_text(encoding="utf-8")
    n = sum(1 for line in text.splitlines() if line.startswith("CAP "))
    snap = next((line for line in text.splitlines() if line.startswith("CAPSNAP")), "")
    print(f"decisions: {n}  {snap}")
    if args.command == "5":
        fails = cohort.check(out)
    else:
        r = cohort.parse(out)
        fails = cohort.run_gates(r, bemf_run.EXPLORE_HOLD_MS) if r else ["incomplete capture"]
    print(f"gates on the capture's run: {'PASS' if not fails else 'FAIL'}")
    for f in fails:
        print(f"  - {f}")
    return 0 if n > 0 else 1


if __name__ == "__main__":
    sys.exit(main())
