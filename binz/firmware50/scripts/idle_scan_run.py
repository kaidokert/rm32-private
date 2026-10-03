"""Flash and capture the bridge-off control (E292).

This is the only fixture in the repo that drives **no** motor: `bin/idle-scan.rs`
never constructs the `Gates` capability, so `moe_on` is uncallable from it. There
is therefore no kill guard here and no stall abort, because there is nothing to
kill -- which is stated explicitly because every other motor script in this tree
is required to have both.

What it collects, in one ~95 s run:

1. `raw*_n` / `raw*_run` with the load removed, in two phases (`EN` low, then
   `EN` high), against the guard's own reference. Decides whether the raw-scan
   excursions E284 built an instrument around are a bus event at all.
2. `gap_over_88us` -- the count of foreground gaps past the no-rewrite window
   `hw::adc` claims for its unsynchronised DMA snapshot. Only the maximum has
   ever been recorded, so the number of potentially torn scans is new.
3. `shunt_*` across the `EN` transition, which is the amplifier-biasing event,
   testing the session-first current under-read.

Usage:  python scripts/idle_scan_run.py [--port COM41] [--no-flash]
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import pathlib
import subprocess
import sys
import time

import serial

REPO = pathlib.Path(__file__).resolve().parent.parent
ELF = REPO / "target" / "thumbv6m-none-eabi" / "release" / "idle-scan"
PROBE = "0483:374b:066CFF343433464757233430"
CHIP = "STM32G071RBTx"

# Two 45 s phases plus two 2048-scan baselines (~0.2 s each) plus the banner.
# Generous, because a short read would truncate the second phase silently.
CAPTURE_S = 115.0

# The run is complete only when this appears. Anything less is a partial capture
# and must not be scored -- the same rule the driving fixture applies.
DONE_MARK = "IDLESCAN done"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=115200)
    ap.add_argument("--label", default="e292-idlescan")
    ap.add_argument("--no-flash", action="store_true")
    args = ap.parse_args()

    if not ELF.exists():
        print(f"no such image: {ELF}\nbuild it: cargo build --release --bin idle-scan")
        return 2
    sha = hashlib.sha256(ELF.read_bytes()).hexdigest().upper()
    print(f"image {ELF.name}  {len(ELF.read_bytes())} bytes  sha256 {sha[:16]}...")

    if not args.no_flash:
        for cmd in (["probe-rs", "download", "--chip", CHIP, "--probe", PROBE, str(ELF)],
                    ["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE]):
            if subprocess.run(cmd, check=False).returncode:
                print("flash failed; refusing to run")
                return 2
        time.sleep(1.0)

    day = datetime.date.today().isoformat()
    out = REPO / "captures" / day / f"{args.label}.txt"
    out.parent.mkdir(parents=True, exist_ok=True)

    lines: list[str] = [f"# elf_sha256 {sha}\n", f"# elf {ELF.name}\n"]
    deadline = time.monotonic() + CAPTURE_S
    done = False
    with serial.Serial(args.port, args.baud, timeout=0.2) as port:
        buf = b""
        while time.monotonic() < deadline and not done:
            buf += port.read(4096)
            while b"\n" in buf:
                raw, buf = buf.split(b"\n", 1)
                line = raw.decode("utf-8", "replace").rstrip("\r") + "\n"
                lines.append(line)
                if line.strip():
                    print("   " + line.rstrip())
                if DONE_MARK in line:
                    done = True
    out.write_text("".join(lines), encoding="utf-8", newline="\n")
    print(f"\nwrote {out}  ({len(lines)} lines)")

    if not done:
        print(f"INCOMPLETE: never saw {DONE_MARK!r}. Not a scorable capture.")
        return 2

    # The safety assertion, checked rather than assumed: this image must end
    # with the bridge demonstrably off, and must never have reported a sag trip
    # with no load on the rail.
    text = "".join(lines)
    problems = []
    if "preflight_passed=1" not in text:
        problems.append("final preflight did not pass")
    if "moe=0" not in text:
        problems.append("MOE not reported clear at exit")
    for phase in ("A_en_low", "B_en_high"):
        if f"phase={phase}" not in text:
            problems.append(f"phase {phase} missing from the capture")
    if problems:
        print("REFUSED to declare this a clean control run:")
        for p in problems:
            print(f"   - {p}")
        return 2
    print("OK: two phases present, bridge reported off at exit.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
