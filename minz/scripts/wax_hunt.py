#!/usr/bin/env python3
"""Analog event hunt: engage, arm the WAX trigger (J), climb to the
event-prone throttle, and record until the firmware's analog black
box fires (>2.4 A cycle sample freezes the WAXWING ring in-ISR) and
auto-dumps ~42 ms of pre-trigger wire truth. Saves the whole session;
the cdump is extracted to <tag>.txt for waxwing.py rendering.

Usage:
    python scripts/wax_hunt.py --amp 51 --tag hunt1
"""

import argparse
import pathlib
import re
import sys
import time

import serial

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--amp", type=int, default=51)
ap.add_argument("--hold", type=float, default=45.0, help="max seconds to wait for trigger")
ap.add_argument("--tag", default="hunt")
args = ap.parse_args()

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
capdir.mkdir(exist_ok=True)

with serial.Serial(args.port, args.baud, timeout=0.05) as p:
    buf = bytearray()

    def send(k, wait=0.3):
        p.write(k.encode())
        time.sleep(wait)
        d = p.read(200000)
        buf.extend(d)
        return d.decode("ascii", errors="replace")

    try:
        send("w", 0.5)
        send("q", 3.0)
        # blank 8 + phys ZC
        for _ in range(6):
            send(",", 0.2)
        for _ in range(8):
            last = send("n", 0.2)
        for _ in range(7):
            if "phys ZC" in send("k", 0.3):
                break
        for _ in range(5):
            send("f", 0.2)
        echo = send("y", 3.0)
        if "ARMED" not in echo:
            sys.exit(f"engage failed: {echo!r}")
        time.sleep(1.5)
        # SWIFT on (M resets to off at boot; verify echo)
        e = send("M", 0.4)
        if "SWIFT" not in e:
            e = send("M", 0.4)
        # arm the analog trigger
        e = send("J", 0.4)
        if "ARMED" not in e:
            e = send("J", 0.4)
        print("engaged, SWIFT on, trigger armed - climbing", flush=True)
        for _ in range(args.amp - 15):
            send("a", 0.12)
        print(f"holding at amp {args.amp}, waiting for trigger...", flush=True)
        t_end = time.monotonic() + args.hold
        fired = False
        while time.monotonic() < t_end:
            d = p.read(200000)
            buf.extend(d)
            if not fired and b"WAX TRIGGER" in buf:
                fired = True
                print("TRIGGER FIRED - collecting dump", flush=True)
                t_end = time.monotonic() + 6.0  # let the cdump finish
        if not fired:
            print("no trigger within hold window", flush=True)
    finally:
        send("y", 0.4)
        send("w", 0.5)
        raw = capdir / f"{args.tag}_session.bin"
        raw.write_bytes(bytes(buf))
        txt = bytes(buf).decode("ascii", errors="replace")
        m = re.search(r"cdump:.*?(?:\r?\n)(?:[!-uz~][^\r\n]*\r?\n)+", txt, re.S)
        if m:
            out = capdir / f"{args.tag}.txt"
            out.write_text(m.group(0))
            print(f"cdump extracted -> {out}")
        else:
            print("no cdump found in session")
