#!/usr/bin/env python3
"""Run the `sag-capture` diagnostic image and save the sag guard's own inputs.

Flash nothing by hand: pass `--elf` and `--flash` so the capture's recorded
hash is necessarily the image that ran. The image is `shell-pwm` with the
sharp-sag guard's inputs recorded per judgement (`bin/sag-capture.rs`,
`src/sagtrace.rs`); its four ISR roots are instruction-identical to
production's, but the recorder costs about 10% of the foreground pass rate, so
its runs are evidence about the guard's inputs and never about production's
loop quality or its stop latency.

Drives the same unjudged 15% warm-up the other diagnostic runners use unless
`--no-warmup`, then the requested key, and saves everything through `SAGEND`
to `captures/sag/<label>.txt`. The run is judged by the ordinary per-run gates
(`cohort.run_gates`), so a trace from an unhealthy run is not passed off as a
measurement of a healthy one.

Usage:
    python scripts/sag_run.py --elf captures/elf/<image>.elf --flash         --label e178-sag475 --command l --rung-duty 475 --pre "+++"
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
ELF = REPO / "target" / "thumbv6m-none-eabi" / "release" / "sag-capture"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default=bemf_run.DEFAULT_PORT)
    ap.add_argument("--label", required=True)
    ap.add_argument("--timeout", type=float, default=130.0)
    ap.add_argument(
        "--command",
        default="5",
        # The gate-4 provocations are admitted too (E182): the positive
        # control that the sag guard still latches *and still freezes the
        # rings* on a given image is `v` -- the 25% bus-sag injection. It must
        # be run at a rung below 50%, because the injection is a duty *step*
        # and at the 50% rung it steps to the duty already commanded.
        choices=["b", "2", "5", "v", "V", "i", "I", "t", "T", "g", "G",
                 "a", "A", "y", "Y", "c", "C", "d", "D", "m", "M",
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
    # Archived under the CRC32 the gate files, the notebook and the
    # predeclaration all use; the SHA-256 prefix this used to use named the same
    # image something else (E188 SS5).
    crc = bemf_run.elf_crc32()
    keep = REPO / "captures" / "elf" / f"{crc}.{args.label}.elf"
    keep.parent.mkdir(parents=True, exist_ok=True)
    if not keep.exists():
        keep.write_bytes(ELF.read_bytes())
    print(f"image crc32={crc} sha256={sha[:8]} archived as {keep.name}")
    out_dir = REPO / "captures" / "sag"
    out_dir.mkdir(parents=True, exist_ok=True)
    warm = out_dir / f"{args.label}-warmup.txt"
    out = out_dir / f"{args.label}.txt"
    # **A colliding label is refused here too** (E188 SS5). `bemf_run.py` gained
    # this after a re-run destroyed nine good captures; this runner writes most
    # of the cohort's runs and had no such check, which left the same hole open.
    clash = [p for p in (out, warm) if p.exists()]
    if clash:
        print("REFUSED: these captures already exist; choose another --label:")
        for c in clash:
            print(f"  {c}")
        return 2
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
                port, out, args.timeout, args.command.encode(), end_marker="SAGEND"
            )
            if not ok:
                print("capture did not complete (no SAGEND)")
                return 1
        finally:
            port.write(bemf_run.STOP)
            port.flush()
    text = out.read_text(encoding="utf-8")
    rows = sum(1 for line in text.splitlines() if line.startswith("SAGROW "))
    snap = next((line for line in text.splitlines() if line.startswith("SAGSNAP")), "")
    print(f"sag blocks: {rows}  {snap}")
    r = cohort.parse(out)
    if r is None:
        print("VERDICT: incomplete run (no report)")
        return 1
    # `cohort.parse` names this `duty`; the old key was never present, so this
    # check silently passed everything (review of E175).
    if args.rung_duty and r.get("duty") not in (None, args.rung_duty):
        print(f"VERDICT: capture ran {r.get('duty')} tenths, not {args.rung_duty}")
        return 1
    fails = cohort.run_gates(r, bemf_run.EXPLORE_HOLD_MS)
    print("VERDICT: " + ("run gates PASS" if not fails else "run gates FAIL: " + "; ".join(fails)))
    return 0 if not fails and rows else 1


if __name__ == "__main__":
    sys.exit(main())
