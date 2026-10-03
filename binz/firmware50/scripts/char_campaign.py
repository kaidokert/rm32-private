#!/usr/bin/env python3
"""Battery characterization campaign (study 2026-10-01): every run, in order.

    python scripts/char_campaign.py [section ...]     # sections: map steps ramps low restart

Resumable: a label whose capture already exists in captures/char/ is skipped,
so an interrupted campaign continues where it stopped. Every run goes through
`char_run.py` on the one measurement image named in captures/char/CURRENT_IMAGE.
A run above 75 % duty is followed by a cool-down pause (no thermal channel on
this bench). Nothing here tunes anything: a stop is recorded and the campaign
moves on.

Profiles (waypoints are duty tenths, slew tenths/s, hold ms; see char_run.py):
* map-DDD: 10 % at close, to DDD at 100 tenths/s, 10 s hold; histograms from 3 s
  after reaching DDD to the end of the hold; recorder at 101 ms.
* step-A-B-k: to A at 100 tenths/s, 4 s hold, then a step to B held 2 s; the
  recorder (3.33 ms) keeps the last 2.1 s, ending 1.5 s after the step.
* ramp-S: 10 -> 100 -> 10 % at slew S (production staircase, 200, 2000, step),
  1 s at each end; the recorder spans the whole schedule.
* low-DDD: to DDD at the production staircase, 30 s hold.
* restart-DDD-k: a step to DDD at the moment the loop closes, 3 s hold; the
  recorder spans the 3 s, and the dump carries the startup time.
"""

from __future__ import annotations

import pathlib
import subprocess
import sys
import time

REPO = pathlib.Path(__file__).resolve().parent.parent
PROD = 65535


def runs():
    out = []
    for d in range(50, 1001, 50):
        out.append(("map", f"map-{d:03d}", 1000, 3000, 0, [(d, 100, 10_000)]))
    steps = [(200, 500), (200, 800), (500, 1000), (100, 1000), (1000, 500), (800, 200), (1000, 100)]
    for a, b in steps:
        for k in (1, 2, 3):
            out.append(("steps", f"step-{a:03d}-{b:03d}-{k}", 33, 0, 1500, [(a, 100, 4000), (b, 0, 2000)]))
    for slew, name, period in ((PROD, "prod", 1430), (200, "200", 175), (2000, "2000", 50), (0, "step", 35)):
        out.append(("ramps", f"ramp-{name}", period, 0, 0, [(1000, slew, 1000), (100, slew, 1000)]))
    for d in (100, 500, 1000):
        for k in (1, 2, 3):
            out.append(("restart", f"restart-{d:03d}-{k}", 48, 0, 0, [(d, 0, 3000)]))
    return out


def low_runs(duties):
    return [("low", f"low-{d:03d}", 3000, 3000, 0, [(d, PROD, 30_000)]) for d in duties]


def run_one(elf: str, label: str, period: int, settle: int, post: int, wps) -> int:
    cap = REPO / "captures" / "char" / f"{label}.txt"
    if cap.exists():
        print(f"skip {label} (exists)")
        return 0
    cmd = [sys.executable, str(REPO / "scripts" / "char_run.py"), "--elf", elf, "--flash", "--label", label,
           "--period", str(period), "--settle", str(settle), "--post", str(post),
           "--wp", *[f"{d},{s},{h}" for d, s, h in wps], "--timeout", "150"]
    print(f"=== {label}", flush=True)
    r = subprocess.run(cmd, capture_output=True, text=True, cwd=REPO / "scripts")
    tail = [l for l in r.stdout.splitlines() if l.startswith(("saved", "REFUSED", "capture did not", "flash failed"))]
    print("   " + (tail[-1] if tail else r.stdout[-300:]) + (" " + r.stderr[-300:] if r.returncode and r.stderr else ""), flush=True)
    if max(d for d, _, _ in wps) > 750:
        time.sleep(45)
    return r.returncode


def main() -> int:
    img = (REPO / "captures" / "char" / "CURRENT_IMAGE").read_text().strip()
    elf = str(next((REPO / "captures" / "elf").glob(f"{img}.char-*.elf")))
    args = sys.argv[1:]
    if args and args[0] == "low":
        # `low DDD DDD ...`: 30 s holds at the given duties (tenths), in order.
        plan = low_runs([int(x) for x in args[1:]])
    else:
        want = set(args) or {"map", "steps", "ramps", "restart"}
        plan = [r for r in runs() if r[0] in want]
    for sec, label, period, settle, post, wps in plan:
        run_one(elf, label, period, settle, post, wps)
    print("### CAMPAIGN DONE", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
