#!/usr/bin/env python3
"""Flash the frozen qualified binz image, run its verified locked sequence, and
capture the telemetry as a reference trace.

Why this exists: firmware50 has no validated rotor-rotation witness. Every
`spun=1` it has ever printed came from a coast peak-to-peak amplitude that
reads the same on a stationary rotor (LAB_NOTEBOOK E031), and `crossings` has
been zero in every capture. So "what does a turning rotor look like in these
fields" has never been measured on this rig -- it has only been guessed, and
guessing is what heated the motor.

The qualified image locks: 35.000005 s powered, `accepted = commutations =
81157` exactly, 394 eHz, every protection counter clear
(`binz/LOW_DUTY_LOCK_EVIDENCE_20260920.md`). Running it produces a known-good
trace to calibrate against.

The command sequence is transcribed from that evidence file. The powered
deadline is shortened by default: the question is what a locked run looks like,
which a few seconds answers, and the motor has already been heated by
firmware50's non-following runs.

Safety: `off` is sent on every exit path including exceptions and Ctrl-C, the
capture is written incrementally so an aborted run still leaves evidence, and
the script never re-flashes firmware50 for you -- do that explicitly.

Usage:
    python scripts/oracle_trace.py --engage-ms 8000
    python scripts/oracle_trace.py --engage-ms 35000 --label oracle_full
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import pathlib
import subprocess
import sys
import time

try:
    import serial  # type: ignore
except ImportError:
    sys.exit("pyserial required: pip install pyserial")

REPO = pathlib.Path(__file__).resolve().parent.parent
BINZ = REPO.parent

ORACLE = (
    BINZ
    / "captures"
    / "reference"
    / "reverse_48k_com_top_high_20260919"
    / "shell-pwm.elf"
)
# From that directory's README.md.
ORACLE_SHA = "0C734C6710D18D7B6E89458F8287CA24C46BF98D791BF8361B7240311E0931B0"

CHIP = "STM32G071RBTx"
PROBE = "0483:374b:066CFF343433464757233430"
PORT = "COM41"
BAUD = 115200

# Transcribed from binz/LOW_DUTY_LOCK_EVIDENCE_20260920.md. `engagems` is
# substituted so the powered window can be shortened.
SEQUENCE = [
    "off",
    "avgnominal",
    "engage0",
    "drivephase60",
    "drivedu61",
    "bemfdu100",
    "drivepwm192",
    "driveobs1",
    "engagems{engage_ms}",
    "drivex1",
    "cap1",
    "du62",
    "catchdu62",
    "live1",
    "run200",
]

STOP = b"off\r"


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def flash() -> None:
    for args in (
        ["probe-rs", "download", "--chip", CHIP, "--probe", PROBE, str(ORACLE)],
        ["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE],
    ):
        done = subprocess.run(args, capture_output=True, text=True)
        if done.returncode != 0:
            raise SystemExit(f"{args[1]} failed: {done.stderr.strip()[:400]}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default=PORT)
    ap.add_argument("--engage-ms", type=int, default=8000)
    ap.add_argument("--label", default="oracle")
    ap.add_argument("--no-flash", action="store_true")
    args = ap.parse_args()

    if not ORACLE.exists():
        raise SystemExit(f"oracle ELF not found: {ORACLE}")
    actual = sha256(ORACLE)
    if actual != ORACLE_SHA:
        raise SystemExit(f"oracle SHA mismatch: {actual} != {ORACLE_SHA}")
    print(f"oracle {actual}")

    if not args.no_flash:
        print("flashing oracle ...")
        flash()
        time.sleep(2.0)

    outdir = REPO / "captures" / datetime.date.today().isoformat()
    outdir.mkdir(parents=True, exist_ok=True)
    out = outdir / f"{args.label}_{args.engage_ms}ms.txt"

    port = serial.Serial(args.port, BAUD, timeout=0.2)
    # The powered window plus startup (align + catch + ramp ~3 s), the coast
    # capture and the post-run dump.
    budget = args.engage_ms / 1000.0 + 40.0

    try:
        time.sleep(0.5)
        port.reset_input_buffer()
        with out.open("w", encoding="utf-8", newline="\n") as fh:
            fh.write(f"# oracle_sha256 {actual}\n")
            fh.write(f"# engage_ms {args.engage_ms}\n")
            fh.write(f"# started {datetime.datetime.now().isoformat(timespec='seconds')}\n")

            for cmd in SEQUENCE:
                line = cmd.format(engage_ms=args.engage_ms)
                fh.write(f"> {line}\n")
                fh.flush()
                print(f"> {line}", flush=True)
                port.write(line.encode() + b"\r")
                port.flush()
                # Let each command be acknowledged before the next; `run200`
                # starts the drive so nothing follows it.
                deadline = time.monotonic() + (budget if line == "run200" else 0.7)
                pending = b""
                while time.monotonic() < deadline:
                    chunk = port.read(4096)
                    if not chunk:
                        if line != "run200":
                            break
                        continue
                    pending += chunk
                    while b"\n" in pending:
                        raw, pending = pending.split(b"\n", 1)
                        text = raw.decode("utf-8", "replace").strip()
                        if not text:
                            continue
                        fh.write(text + "\n")
                        fh.flush()
                        print("   " + text, flush=True)
                        if text.startswith("DONE "):
                            deadline = time.monotonic() + 3.0
    except KeyboardInterrupt:
        print("\ninterrupted", flush=True)
    finally:
        try:
            for _ in range(4):
                port.write(STOP)
                port.flush()
                time.sleep(0.15)
        finally:
            port.close()

    print(f"\ncapture: {out}")
    print("firmware50 is NOT restored; flash it back explicitly when done.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
