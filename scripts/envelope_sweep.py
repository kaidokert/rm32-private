#!/usr/bin/env python3
"""Envelope re-baseline: staircase dwell sweep with per-level lock stats.

Run after any change that could move the retention wall. Decodes the zct
stream per throttle level: window count, median z, jitter, OldRoutine
(fall) windows, deaf windows (z > 2*ci at load). Corrupt wire records
(text interleave from bench echoes) are filtered by sanity bounds.
"""
import argparse
import statistics
import struct
import subprocess
import sys
import time

import serial

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"
CUR_MA = 26.855


def reset_board():
    subprocess.run(
        ["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE],
        capture_output=True,
    )
    time.sleep(2)


def parse(raw):
    comms, i, pend = [], 0, None
    while i + 15 <= len(raw):
        if raw[i] == 0x5B and raw[i + 1] == 0xA9:
            fl = raw[i + 2]
            v = struct.unpack_from("<6H", raw, i + 3)
            pend = (fl & 7, bool(fl & 8), bool(fl & 0x80)) + v
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


def sane(z, p):
    """Reject text-interleave corrupted records."""
    return z[3] < 8000 and z[6] < 1200 and p[6] < 4000 and p[1] < 500


def level_stats(comms, marks, name, t0, t1):
    win = [
        (z, p)
        for (z, p), tk in zip(comms, marks)
        if t0 <= tk < t1 and sane(z, p)
    ]
    if len(win) < 30:
        print(f"  {name:>4}: {len(win)} w (too few)")
        return
    zs = [z[3] for z, _ in win]
    med = statistics.median(zs)
    sd = statistics.pstdev(zs) / 2
    old = sum(1 for z, _ in win if z[1])
    deaf = sum(1 for z, _ in win if z[3] > 2 * z[4] and z[4] > 100)
    cur = [p[3] * CUR_MA / 1000 for _, p in win if p[3] > 0]
    imed = statistics.median(cur) if cur else 0.0
    fe_hz = 2e6 / (6 * med) if med else 0
    print(
        f"  {name:>4}: {len(win):6d} w  z_med={med:5.0f} ({fe_hz:5.0f} Hz e)"
        f"  sd={sd:5.1f}us  old={old}  deaf={deaf}  I={imed:.2f}A"
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument(
        "--levels",
        default="15:4,30:4,50:8,60:8,70:8,80:6",
        help="pct:secs comma list",
    )
    ap.add_argument("--outfile", default="envelope.bin")
    ap.add_argument("--no-inj", action="store_true")
    ap.add_argument(
        "--grad",
        type=int,
        default=0,
        help="walk between levels in steps of this many %% (0 = hard steps)",
    )
    a = ap.parse_args()
    levels = [(int(s.split(":")[0]), float(s.split(":")[1])) for s in a.levels.split(",")]

    reset_board()
    p = serial.Serial(a.port, 2_000_000, timeout=0.05)
    raw = b""
    stamps = []  # (byte_offset, level_name)

    last_poll = [0.0]

    def dwell(pct, secs):
        nonlocal raw
        t0 = time.time()
        while time.time() - t0 < secs:
            p.write(f"{pct}\n".encode())
            p.flush()
            now = time.time()
            if now - last_poll[0] > 0.3:
                p.write(b"i")  # info line: vbat/iraw/duty/guard timeline
                p.flush()
                last_poll[0] = now
            raw += p.read(65536)
            time.sleep(0.10)

    try:
        dwell(0, 2)
        p.write(b"Z")
        p.flush()
        time.sleep(0.2)
        raw += p.read(65536)
        armed_j = False
        cur_pct = 0
        for pct, secs in levels:
            if pct >= 40 and not armed_j and not a.no_inj:
                p.write(b"J")
                p.flush()
                armed_j = True
            if a.grad and pct > cur_pct:
                # stick-like walk: small steps, each held long enough for
                # the two-frame confirmation (2 sends) plus settling
                step = a.grad
                x = cur_pct + step
                while x < pct:
                    dwell(x, 0.45)
                    x += step
            stamps.append((len(raw), pct))
            dwell(pct, secs)
            cur_pct = pct
        stamps.append((len(raw), -1))
        time.sleep(0.5)
        raw += p.read(300000)
    finally:
        p.write(b"w")
        p.flush()
        p.close()

    with open(a.outfile, "wb") as f:
        f.write(raw)

    # Map each record to a byte offset to bin it into levels.
    comms, marks, i, pend, pend_off = [], [], 0, None, 0
    while i + 15 <= len(raw):
        if raw[i] == 0x5B and raw[i + 1] == 0xA9:
            fl = raw[i + 2]
            v = struct.unpack_from("<6H", raw, i + 3)
            pend = (fl & 7, bool(fl & 8), bool(fl & 0x80)) + v
            pend_off = i
            i += 15
        elif raw[i] == 0x5B and raw[i + 1] == 0xA6:
            fl = raw[i + 2]
            fe, en, tl, cur = struct.unpack_from("<4H", raw, i + 3)
            gc, pr = raw[i + 11], raw[i + 12]
            la = struct.unpack_from("<H", raw, i + 13)[0]
            if pend is not None and pend[0] == (fl & 7):
                comms.append((pend, (fe, en, tl, cur, gc, pr, la)))
                marks.append(pend_off)
            pend = None
            i += 15
        else:
            i += 1

    print(f"total {len(comms)} windows -> {a.outfile}")
    for k in range(len(stamps) - 1):
        off0, pct = stamps[k]
        off1, _ = stamps[k + 1]
        # skip the first second of each level (transition)
        level_stats(comms, marks, f"{pct}%", off0, off1)

    # Info-line timeline (from 'i' polls): the decision axis — duty vs
    # dmax vs vbat vs current, with byte offsets mapping into levels.
    import re

    print("-- info timeline (offset, level markers at", [s[0] for s in stamps], ")")
    for m in re.finditer(rb"i step=\S+ [^\n]*", raw):
        line = m.group(0).decode(errors="replace")
        kv = dict(
            (p.split("=")[0], p.split("=")[1])
            for p in line.split()
            if "=" in p and p.count("=") == 1
        )
        print(f"  @{m.start():7d} {line}")
    for m in re.finditer(rb"!! BENCH KILL[^\n]*", raw):
        print(f"  @{m.start():7d} {m.group(0).decode(errors='replace')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
