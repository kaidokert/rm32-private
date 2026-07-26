#!/usr/bin/env python3
"""Freeze-on-fall spiral hunt: ramp to 50%, wait for the firmware's
early-accept-spiral trigger to freeze the zct ring, decode the tail.

Firmware side (isr_handlers.rs): N consecutive windows with
z + ci/8 < ci while zc>=10000 && duty>500 -> bench_zct::freeze().
Frozen ring stops recording, so the wire tail = the onset, un-garbaged.

Requires: zctrace+benchuart build flashed, Z toggled on after boot.
Saves raw capture per rep to --outdir (default scratchpad-less: ./captures).
"""
import argparse
import struct
import subprocess
import sys
import time

import serial

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"
CUR_MA = 26.855  # injected ADC raw -> mA


def reset_board():
    subprocess.run(
        ["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE],
        capture_output=True,
    )
    time.sleep(2)


def parse(raw):
    """Pair zct (5B A9) + probe (5B A6) records."""
    comms, i, pend = [], 0, None
    while i + 15 <= len(raw):
        if raw[i] == 0x5B and raw[i + 1] == 0xA9:
            fl = raw[i + 2]
            v = struct.unpack_from("<6H", raw, i + 3)
            pend = (fl & 7, bool(fl & 0x80)) + v  # step, batching, z,ci,w,duty,tenkhz,avg
            i += 15
        elif raw[i] == 0x5B and raw[i + 1] == 0xA6:
            fl = raw[i + 2]
            fe, en, tl, cur = struct.unpack_from("<4H", raw, i + 3)
            gc, pr = raw[i + 11], raw[i + 12]
            la = struct.unpack_from("<H", raw, i + 13)[0]
            if pend is not None and pend[0] == (fl & 7):
                comms.append((pend, (fe, en, tl, cur, gc, pr, la)))
            pend = None
            i += 15
        else:
            i += 1
    return comms


def print_tail(comms, n):
    print("  s b   z    ci    w  duty | fe    en   tl    la  I_A  gc pr")
    for z, p in comms[-n:]:
        fe = "none " if p[0] == 0xFFFF else f"{p[0]:5d}"
        tl = "none " if p[2] == 0xFFFF else f"{p[2]:5d}"
        la = "none " if p[6] == 0xFFFF else f"{p[6]:5d}"
        b = "B" if z[1] else " "
        print(
            f"  {z[0]} {b} {z[2]:5d} {z[3]:5d} {z[4]:4d} {z[5]:4d}"
            f" | {fe} {p[1]:4d} {tl} {la} {p[3]*CUR_MA/1000:5.2f} {p[4]:2d} {p[5]:2d}"
        )


def stats(comms):
    """Deaf events (z way past due at load) + late TIM16 fires."""
    deaf = sum(1 for z, _ in comms if z[2] > 900 and z[5] > 400)
    late = sum(
        1
        for _, p in comms
        if p[2] != 0xFFFF and p[6] != 0xFFFF and p[2] > p[6] + 60
    )
    return deaf, late


def run_rep(rep, port, outdir, tail):
    reset_board()
    p = serial.Serial(port, 2_000_000, timeout=0.05)
    raw = b""

    def dwell(pct, secs):
        nonlocal raw
        t0 = time.time()
        while time.time() - t0 < secs:
            p.write(f"{pct}\n".encode())
            p.flush()
            raw += p.read(65536)
            time.sleep(0.10)

    try:
        dwell(0, 2)
        p.write(b"Z")
        p.flush()
        time.sleep(0.2)
        raw += p.read(65536)
        dwell(15, 5)
        dwell(25, 3)
        p.write(b"J")  # arm injected current sampler at speed (boot-arm kills climbs)
        p.flush()
        dwell(40, 4)
        dwell(50, 14)
        time.sleep(0.5)
        raw += p.read(300000)
    finally:
        p.write(b"w")
        p.flush()
        p.close()

    fn = f"{outdir}/spiral{rep}.bin"
    with open(fn, "wb") as f:
        f.write(raw)
    comms = parse(raw)
    last = comms[-1][0] if comms else None
    frozen = last is not None and last[6] > 500 and last[4] < 600
    tag = "** SPIRAL FROZEN **" if frozen else ""
    deaf, late = stats(comms)
    print(
        f"=== rep {rep}: {len(comms)} w; deaf={deaf} late_fire={late};"
        f" tail duty={last[6] if last else '-'}"
        f" ci={last[4] if last else '-'} {tag} -> {fn}",
        flush=True,
    )
    if frozen:
        print_tail(comms, tail)
    return frozen


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--reps", type=int, default=4)
    ap.add_argument("--tail", type=int, default=120)
    ap.add_argument("--outdir", default=".")
    ap.add_argument("--stop-on-freeze", action="store_true")
    a = ap.parse_args()
    hits = 0
    for rep in range(1, a.reps + 1):
        if run_rep(rep, a.port, a.outdir, a.tail):
            hits += 1
            if a.stop_on_freeze:
                break
    print(f"freezes: {hits}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
