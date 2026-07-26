#!/usr/bin/env python3
"""Parity Dossier campaign runner — one command per (program, firmware).

Programs (the dossier matrix, PSU-safe subset — slam rows are
battery-gated, see project memory):
  ladder    steady holds 15..100%, 45 s each
  climb     gradual 0->100->0 at 3%/0.45s
  steps     moderate step set (20-50, 30-60, 50-70, 60-40, 40-20)
  boundary  55-65% boundary dwell cycling
  soak      10-minute mixed hold (50/70 alternating 60 s)

Firmware is whatever is flashed; pass --fw rm32|clone so the runner
uses the right toggles (Z-verify loop for the clone, J current arm for
rm32) and the parser expects the right record mix. Captures land in
--outdir as <fw>_<program>_<rep>.bin plus a metrics JSON per run.

ABAB protocol: run e.g.
  dossier.py ladder --fw rm32  ; flash clone (dossier-clone-ref!)
  dossier.py ladder --fw clone ; flash rm32 ; repeat
Clone card: reset clears its reset-only kill latch (runner resets per
rep); stage via 20% (cold jumps >50% break its start detection); its
deadman needs the 10 Hz re-sends the dwell loop already does.
"""
import argparse
import json
import statistics
import struct
import subprocess
import sys
import time

import serial

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"
CUR_MA = 26.855
VBAT_MV = 7.517


def reset_board():
    subprocess.run(
        ["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE],
        capture_output=True,
    )
    time.sleep(2)


class Bench:
    def __init__(self, port):
        self.p = serial.Serial(port, 2_000_000, timeout=0.05)
        self.raw = b""
        self.marks = []  # (byte_offset, label)

    def mark(self, label):
        self.marks.append((len(self.raw), label))

    def dwell(self, pct, secs):
        t0 = time.time()
        while time.time() - t0 < secs:
            self.p.write(f"{pct}\n".encode())
            self.p.flush()
            self.raw += self.p.read(65536)
            time.sleep(0.1)

    def cmd(self, b):
        self.p.write(b)
        self.p.flush()
        time.sleep(0.2)
        self.raw += self.p.read(8192)

    def z_verify(self):
        """Toggle trace on; resend until 5B A9 records flow (clone's
        ISR RX eats single bytes)."""
        for _ in range(5):
            self.cmd(b"Z")
            m = len(self.raw)
            self.dwell(15, 1.5)
            if self.raw[m:].count(0xA9) > 3:
                return True
        return False

    def finish(self):
        time.sleep(0.5)
        self.raw += self.p.read(400000)
        self.cmd(b"i")
        self.p.write(b"w")
        self.p.flush()
        self.p.close()


def parse_zct(raw):
    """5B A9 rows (both firmwares) -> (step, old, batching, z, ci, w,
    duty, tenkhz, avg) with byte offsets."""
    recs, offs, i = [], [], 0
    n = len(raw)
    while i + 15 <= n:
        if raw[i] == 0x5B and raw[i + 1] == 0xA9:
            fl = raw[i + 2]
            v = struct.unpack_from("<6H", raw, i + 3)
            r = (fl & 7, bool(fl & 8), bool(fl & 0x40 or fl & 0x80)) + v
            if r[4] < 8000 and r[6] < 2100:  # sanity: ci, duty
                recs.append(r)
                offs.append(i)
            i += 15
        elif raw[i] == 0x5B and raw[i + 1] == 0xA6:
            i += 15  # rm32 probe row — skip whole record so its payload
            # bytes can't fake an A9 header (phantom-record class)
        else:
            i += 1
    return recs, offs


def seg_metrics(recs):
    """Metric bundle for a slice of records."""
    if len(recs) < 50:
        return {"n": len(recs)}
    zs = [r[3] for r in recs if r[4] < 450]
    deaf = sum(
        1
        for k in range(1, len(recs))
        if recs[k - 1][4] < 300 and recs[k][3] > int(2.2 * recs[k - 1][4])
    )
    out = {"n": len(recs), "deaf": deaf}
    if zs:
        med = statistics.median(zs)
        out.update(
            z_med=round(med),
            fe_hz=round(2e6 / (6 * med)) if med else 0,
            sd_us=round(statistics.pstdev(zs) / 2, 1),
            lock_frac=round(len(zs) / len(recs), 4),
        )
    return out


def chop_episodes(recs):
    eps, hist, k = [], [], 0
    while k < len(recs):
        d = recs[k][6]
        hist.append(d)
        if len(hist) > 25:
            hist.pop(0)
        plat = max(hist)
        if plat > 300 and d < plat * 0.7:
            t, floor, j = 0, d, k
            while j < len(recs) and recs[j][6] < plat * 0.9:
                t += recs[j][3]
                floor = min(floor, recs[j][6])
                j += 1
            eps.append({"ms": round(t / 2000.0, 1), "floor": floor, "plateau": plat})
            hist = []
            k = j
        else:
            k += 1
    return eps


def info_counters(raw):
    import re

    out = {}
    for pat, key in ((rb"dsy=(\d+)", "dsy"), (rb"otrip=(\d+)", "otrip"), (rb"bt=(\d+)", "bt")):
        m = re.findall(pat, raw)
        if m:
            out[key] = int(m[-1])
    k = raw.find(b"!! BENCH")
    out["kill"] = raw[k : k + 70].decode(errors="replace").strip() if k >= 0 else None
    return out


PROGRAMS = {}


def program(fn):
    PROGRAMS[fn.__name__] = fn
    return fn


@program
def ladder(b):
    b.dwell(0, 1.5)
    for pct in (15, 20, 30, 40, 50, 60, 70, 80, 90, 100):
        b.mark(f"{pct}")
        b.dwell(pct, 45)


@program
def climb(b):
    b.dwell(0, 1.5)
    b.mark("up")
    x = 3
    while x <= 100:
        b.dwell(x, 0.45)
        x += 3
    b.dwell(100, 3)
    b.mark("down")
    while x >= 3:
        b.dwell(x, 0.45)
        x -= 3


@program
def steps(b):
    b.dwell(0, 1.5)
    b.dwell(20, 3)
    for lo, hi in ((20, 50), (30, 60), (50, 70), (60, 40), (40, 20)):
        b.mark(f"{lo}-{hi}")
        b.dwell(lo, 3)
        b.dwell(hi, 4)
    b.dwell(20, 2)


@program
def boundary(b):
    b.dwell(0, 1.5)
    b.dwell(20, 2)
    b.dwell(40, 3)
    for _ in range(6):
        b.mark("55")
        b.dwell(55, 8)
        b.mark("65")
        b.dwell(65, 8)


@program
def soak(b):
    b.dwell(0, 1.5)
    b.dwell(20, 2)
    b.dwell(40, 3)
    for _ in range(5):
        b.mark("50")
        b.dwell(50, 60)
        b.mark("70")
        b.dwell(70, 60)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("program", choices=sorted(PROGRAMS))
    ap.add_argument("--fw", choices=("rm32", "clone"), required=True)
    ap.add_argument("--rep", type=int, default=1)
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--outdir", default="captures/dossier")
    a = ap.parse_args()

    import os

    os.makedirs(a.outdir, exist_ok=True)
    reset_board()
    b = Bench(a.port)
    try:
        b.dwell(0, 1.0)
        if a.fw == "clone":
            if not b.z_verify():
                print("WARN: clone trace not confirmed")
        else:
            b.cmd(b"Z")
            b.cmd(b"J")
        PROGRAMS[a.program](b)
    finally:
        b.finish()

    stem = f"{a.outdir}/{a.fw}_{a.program}_{a.rep}"
    with open(stem + ".bin", "wb") as f:
        f.write(b.raw)

    recs, offs = parse_zct(b.raw)
    bounds = b.marks + [(len(b.raw), "end")]
    result = {
        "fw": a.fw,
        "program": a.program,
        "rep": a.rep,
        "records": len(recs),
        "counters": info_counters(b.raw),
        "segments": {},
    }
    import bisect

    for (o0, lbl), (o1, _) in zip(bounds, bounds[1:]):
        k0 = bisect.bisect_left(offs, o0)
        k1 = bisect.bisect_left(offs, o1)
        seg = recs[k0:k1]
        m = seg_metrics(seg)
        m["chop"] = chop_episodes(seg)
        key = lbl if lbl not in result["segments"] else f"{lbl}#{k0}"
        result["segments"][key] = m
    with open(stem + ".json", "w") as f:
        json.dump(result, f, indent=1)
    print(json.dumps(result["counters"]))
    for k, v in result["segments"].items():
        chop = len(v.get("chop", []))
        print(
            f"  {k:>7}: n={v.get('n',0):6d} fe={v.get('fe_hz','-'):>5} "
            f"sd={v.get('sd_us','-'):>6} deaf={v.get('deaf','-')} chop={chop}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
