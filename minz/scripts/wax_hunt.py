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
ap.add_argument("--force", action="store_true",
                help="send a plain `j` after settling instead of arming the "
                "trigger — deterministic dump-under-load test at any amp")
args = ap.parse_args()

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
capdir.mkdir(exist_ok=True)

with serial.Serial(args.port, args.baud, timeout=0.05) as p:
    # The dump bursts ~21 kB at 200 kB/s. The default Windows/FTDI RX
    # buffer (~8 kB) silently drops the excess during any host sleep —
    # that, not firmware, was the "truncated dump" saga (4 hunts).
    try:
        p.set_buffer_size(rx_size=1 << 20)
    except Exception:
        pass
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
        # Climb FIRST, then arm. Two hard-won ordering rules:
        # (1) transit spikes during the climb fire the trigger early
        #     (observed: armed at 15, fired at amp 45 mid-climb);
        # (2) any queued keypress dispatched while the chunked dump
        #     is streaming ABORTS it mid-payload (observed twice:
        #     payload stops exactly where the amp=NN echoes appear).
        # So: reach the hold amp, settle, arm with the last key we
        # send, then wait in complete key silence.
        print("engaged, SWIFT on - climbing", flush=True)
        for _ in range(args.amp - 15):
            send("a", 0.12)
        time.sleep(1.0)
        t_end = time.monotonic() + args.hold
        fired = done = False
        if args.force:
            p.write(b"j")  # deterministic dump; collect in key silence
            fired = True
            t_end = time.monotonic() + 30.0
            print(f"forced `j` at amp {args.amp}, collecting...", flush=True)
        else:
            # No send()/sleep here: at event-prone amps the trigger
            # fires within the echo-wait and the host sleeps through
            # the 200 kB/s dump burst. Write the key and drop straight
            # into the read loop; the arm echo lands in the stream.
            p.write(b"J")
            print(f"holding at amp {args.amp}, trigger armed, waiting in key silence...",
                  flush=True)
        chunks = []  # (t, nbytes) arrival log — distinguishes a
        #              firmware output gap from wire/host byte loss
        while time.monotonic() < t_end:
            d = p.read(200000)
            if d:
                chunks.append((time.monotonic(), len(d)))
            buf.extend(d)
            if not fired and b"WAX TRIGGER" in buf:
                fired = True
                print("TRIGGER FIRED - collecting dump", flush=True)
                # The dump runs in main-loop idle slices and can take
                # many seconds under full CL ISR load at high amp —
                # killing before the `end` marker truncates the
                # payload (learned at amp 48: 246/256 lines, then the
                # finally-kill ate the tail).
                t_end = time.monotonic() + 30.0
            if fired and b"\nend" in buf:
                done = True
                print("dump complete", flush=True)
                break
        if not fired:
            print("no trigger within hold window", flush=True)
        elif not done:
            print("WARNING: dump did not reach 'end' - payload truncated", flush=True)
        if chunks:
            t0 = chunks[0][0]
            tot = sum(n for _, n in chunks)
            print(f"arrival: {tot} bytes in {len(chunks)} chunks over "
                  f"{chunks[-1][0] - t0:.2f}s", flush=True)
            prev = t0
            for t, n in chunks:
                if t - prev > 0.3:
                    print(f"  GAP {t - prev:.2f}s before chunk at t+{t - t0:.2f}s "
                          f"({n} B)", flush=True)
                prev = t
        # Reboot probe: an IWDG reset mid-dump leaves a responsive
        # firmware with zeroed counters/uptime — visible in `i`.
        probe = send("i", 1.5)
        for ln in probe.splitlines():
            if "irq/s" in ln or "vbat=" in ln or "cl:" in ln:
                print(f"post-dump probe: {ln.strip()}", flush=True)
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
