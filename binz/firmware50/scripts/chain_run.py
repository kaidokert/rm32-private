#!/usr/bin/env python3
"""Run the `chain-capture` diagnostic image and save its timing chain.

Flash `chain-capture` first (`shell-pwm` with both motor roots recording the
commutation timing chain; see `bin/chain-capture.rs` and `src/chain.rs`).
This drives the same unjudged 15% warm-up the other diagnostic runners use --
the current proxy's zero follows the driver's temperature (E092/E093) -- then
the requested key, and saves everything through `CHAINEND` to
`captures/chain/<label>.txt` with the image's SHA-256 in the header.

The run is judged by the ordinary per-run gates (`cohort.run_gates`), so a
chain recorded from an unhealthy run is not passed off as a measurement of a
healthy one. A recording image is slower in both roots (E154), so its runs are
evidence about timing *structure*, never about production loop quality.

Usage:
    python scripts/chain_run.py --label e154-chain25 --command 5
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys
import time

import serial  # type: ignore

import bemf_run
import cohort

REPO = pathlib.Path(__file__).resolve().parent.parent
ELF = REPO / "target" / "thumbv6m-none-eabi" / "release" / "chain-capture"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default=bemf_run.DEFAULT_PORT)
    ap.add_argument("--label", required=True)
    ap.add_argument("--timeout", type=float, default=130.0)
    ap.add_argument(
        "--command",
        default="5",
        choices=["b", "2", "5", "a", "A", "y", "Y", "c", "C", "d", "D", "m", "M",
                 "e", "E", "j", "J", "l", "L", "Z", "x"],
        help="the shell key to drive; the rung keys are the same as the "
        "fixture's (`J` = 37.5%% qualified window, `j` = its explore window)",
    )
    ap.add_argument(
        "--rung-duty",
        type=int,
        default=0,
        help="the duty (tenths) `l`/`L` are set to; checked against the capture",
    )
    ap.add_argument("--no-warmup", action="store_true")
    ap.add_argument("--elf", default="", help="the recording image to run (and to hash)")
    ap.add_argument("--flash", action="store_true", help="program --elf before the run")
    ap.add_argument(
        "--pre",
        default="",
        help="keys to send before the run and not record, e.g. '+' to step the "
        "climb duty up one rung (each is echoed by the shell, so the capture "
        "still says which duty actually ran)",
    )
    args = ap.parse_args()
    if args.elf:
        globals()["ELF"] = pathlib.Path(args.elf)
    bemf_run.ELF = ELF
    # ENV-26: refuse an existing label BEFORE flashing or driving.
    _early = REPO / "captures" / "chain" / f"{args.label}.txt"
    if _early.exists():
        sys.exit(f"REFUSED: {_early} exists; choose another --label")
    if args.flash:
        probe = "0483:374b:066CFF343433464757233430"
        for cmd in (["probe-rs", "download", "--chip", "STM32G071RBTx", "--probe", probe, str(ELF)],
                    ["probe-rs", "reset", "--chip", "STM32G071RBTx", "--probe", probe]):
            if subprocess.run(cmd, check=False).returncode:
                print("flash failed; refusing to run")
                return 2
        time.sleep(1.0)
    # Archive the image being run, keyed by its own hash, *before* driving it.
    # Two of step 5's six rungs ran on images that were rebuilt before they
    # could be archived, so their ELFs are unrecoverable and the entry had to
    # say so (the independent review of E161 caught the pooling). This makes
    # that impossible to repeat.
    sha = bemf_run.elf_sha256()
    keep = REPO / "captures" / "elf" / f"{sha[:8]}.{args.label}.elf"
    keep.parent.mkdir(parents=True, exist_ok=True)
    if not keep.exists():
        keep.write_bytes(ELF.read_bytes())
    print(f"image {sha[:8]} archived as {keep.name}")
    out_dir = REPO / "captures" / "chain"
    out_dir.mkdir(parents=True, exist_ok=True)
    warm = out_dir / f"{args.label}-warmup.txt"
    out = out_dir / f"{args.label}.txt"
    # ENV-26: refuse to overwrite. A re-used label silently destroyed the
    # origin-only capture env26-chain725-1; captures are evidence.
    if out.exists():
        sys.exit(f"REFUSED: {out} exists; choose another --label")
    with serial.Serial(args.port, 115200, timeout=0.2) as port:
        try:
            if not args.no_warmup:
                print("== warm-up 15% (unjudged)")
                if not bemf_run.capture_one(port, warm, 85.0, b"b"):
                    print("warm-up did not complete")
                    return 1
                time.sleep(20.0)
            for key in args.pre:
                print(f"== pre-key {key!r} (not recorded)")
                port.write(key.encode())
                port.flush()
                time.sleep(1.0)
                print("   " + port.read(400).decode(errors="replace").strip())
            print(f"== key {args.command} with the chain recorded -> {out}")
            ok = bemf_run.capture_one(
                port, out, args.timeout, args.command.encode(), end_marker="CHAINEND"
            )
            if not ok:
                print("capture did not complete (no CHAINEND)")
                return 1
        finally:
            port.write(bemf_run.STOP)
            port.flush()
    text = out.read_text(encoding="utf-8")
    rows = sum(1 for line in text.splitlines() if line.startswith("CHAIN "))
    snap = next((line for line in text.splitlines() if line.startswith("CHAINSNAP")), "")
    print(f"chain rows: {rows}  {snap}")
    r = cohort.parse(out)
    if r is None:
        print("VERDICT: incomplete run (no report)")
        return 1
    if args.rung_duty and r.get("target_duty_tenths") not in (None, args.rung_duty):
        print(f"VERDICT: capture ran {r.get('target_duty_tenths')} tenths, not {args.rung_duty}")
        return 1
    fails = cohort.run_gates(r, bemf_run.EXPLORE_HOLD_MS)
    print("VERDICT: " + ("run gates PASS" if not fails else "run gates FAIL: " + "; ".join(fails)))
    return 0 if not fails and rows else 1


if __name__ == "__main__":
    sys.exit(main())
