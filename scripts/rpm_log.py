#!/usr/bin/env python3
"""Log RPM + current + voltage vs time during a 100% hold, then plot.

Purpose: capture the reported ~4s audible beat in DATA (or show it
gone after the bench_lib poll fix). Samples info at max rate (~2.2 Hz
with round-trips — 9+ points per 4 s period), computes mechanical RPM
from ci (f_e = 2e6/(6*ci); rpm = f_e*60/pole_pairs), writes a CSV and
a two-panel PNG (RPM, current+voltage) sharing the time axis.

Usage: rpm_log.py [port] [pct] [secs] [--out captures/rpm_log]
"""
import csv
import sys
import time

from bench_lib import Bench, reset_board

POLE_PAIRS = 7  # 14-pole bench motor


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    port = args[0] if len(args) > 0 else "COM41"
    pct = int(args[1]) if len(args) > 1 else 100
    secs = float(args[2]) if len(args) > 2 else 20.0
    stem = "../captures/rpm_log"
    for a in sys.argv[1:]:
        if a.startswith("--out="):
            stem = a.split("=", 1)[1]

    identity, _ = reset_board(port)
    if identity != "rm32":
        print(f"ABORT identity={identity}")
        return 1
    rows = []
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            print("engage failed")
            return 1
        b.cmd(b"T", settle=0.25)
        for p in (70, 80, 90, 96, pct):
            if p <= pct:
                b.hold(p, 2.2)
        print(f"== logging {pct}% for {secs:.0f}s ==")
        t0 = time.time()
        while time.time() - t0 < secs:
            b.hold(pct, 0.05)          # keep stream hot between samples
            inf = b.info(retries=1)
            if inf is None or not inf.ci:
                continue
            t = time.time() - t0
            fe = 2e6 / (6 * inf.ci)
            rpm = fe * 60 / POLE_PAIRS
            rows.append((round(t, 2), inf.ci, round(rpm), round(inf.amps, 2),
                         round(inf.volts, 2), inf.dsy, int(inf.running)))
        end = b.info()
        print(f"end: {end}")
    print(f"samples: {len(rows)}")

    with open(stem + ".csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["t_s", "ci", "rpm", "amps", "volts", "dsy", "running"])
        w.writerows(rows)

    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    t = [r[0] for r in rows]
    rpm = [r[2] for r in rows]
    amps = [r[3] for r in rows]
    volts = [r[4] for r in rows]
    fig, ax = plt.subplots(2, 1, figsize=(14, 7), sharex=True)
    ax[0].plot(t, rpm, lw=1.4, color="#2b6cb0", marker="o", ms=3)
    ax[0].set_ylabel("mechanical RPM")
    ax[0].set_title(f"{pct}% hold, {secs:.0f}s — RPM / current / voltage vs time "
                    f"(n={len(rows)}, ~{len(rows)/secs:.1f} Hz sampling)")
    ax[0].grid(alpha=0.3)
    ax[1].plot(t, amps, lw=1.4, color="#c05621", marker="o", ms=3, label="current (A)")
    ax[1].set_ylabel("current (A)", color="#c05621")
    ax[1].grid(alpha=0.3)
    ax1v = ax[1].twinx()
    ax1v.plot(t, volts, lw=1.2, color="#276749", marker="s", ms=2, label="vbat (V)")
    ax1v.set_ylabel("vbat (V)", color="#276749")
    ax[1].set_xlabel("time (s)")
    fig.tight_layout()
    out = stem + ".png"
    fig.savefig(out, dpi=110)
    print(f"wrote {stem}.csv and {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
