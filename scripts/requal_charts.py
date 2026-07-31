#!/usr/bin/env python3
"""Poll-free re-qual WITH clone-style charts via the onboard recorder.

Same three segments as requal_quiet (10% ladder, 2% ladder x91, slams),
zero polls during measurement. The firmware's flight recorder samples
(ci, mA, mV) every 0.5s on its own; a single 'B' dump afterwards yields
the full time series -> per-segment RPM/current/voltage PNGs, exactly
the clone reference's chart set, without the print disturbance.

Usage: requal_charts.py [port]
Outputs: captures/requal_sr.csv + captures/requal_seg_*.png + summary.
"""
import csv
import re
import sys
import time

from bench_lib import Bench, reset_board

POLE_PAIRS = 7


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    identity, _ = reset_board(port)
    if identity != "rm32":
        print(f"ABORT identity={identity}")
        return 1
    marks = []  # (t_wall, label)
    t_run0 = None
    with Bench(port) as b:
        if not b.engage_from_stop(30):
            print("engage failed")
            return 1

        def boundary():
            inf = b.info()
            return {"cm": int(inf.raw.get("cm", 0)),
                    "exc": int(inf.raw.get("exc", 0)),
                    "dsy": inf.dsy, "volts": inf.volts}

        t_run0 = time.time()
        segs = []

        def seg(name, profile):
            s0 = boundary()
            marks.append((time.time() - t_run0, name + ":start"))
            for pct, dwell in profile:
                b.hold(pct, dwell)
            b.hold(30, 1.5)
            marks.append((time.time() - t_run0, name + ":end"))
            s1 = boundary()
            r = {"name": name, "comms": s1["cm"] - s0["cm"],
                 "exc": s1["exc"] - s0["exc"], "dsy": s1["dsy"] - s0["dsy"]}
            segs.append(r)
            print(f"  {name}: {r}")

        print("== A: 10% ladder ==")
        seg("10pct", [(p, 5.0) for p in range(10, 101, 10)])
        print("== B: 2% ladder x91 ==")
        seg("2pct", [(p, 1.5) for p in
                     list(range(10, 101, 2)) + list(range(98, 9, -2))])
        print("== C: slams ==")
        seg("slams", [(10, 2.0), (100, 3.0), (10, 2.0),
                      (10, 0.5), (100, 3.0), (10, 2.0)])

        # single post-run recorder dump
        n0 = len(b.buf)
        b.p.reset_input_buffer()
        b.p.write(b"B")
        b.p.flush()
        raw = bytearray()
        t0 = time.time()
        while time.time() - t0 < 20.0:
            c = b.p.read(200000)
            if c:
                raw += c
            if b"SR END" in raw:
                break
        txt = raw.decode("latin1", errors="replace")
    rows = [(int(a), int(m), int(v)) for a, m, v in
            re.findall(r"^SR (\d+) (\d+) (\d+)$", txt, re.M)]
    print(f"recorder samples: {len(rows)}")
    if not rows:
        print("no recorder data")
        return 1

    # time axis: dump order is oldest-first, 0.5s apart, ending ~now.
    n = len(rows)
    ts = [(-0.5 * (n - 1 - k)) + (time.time() - t_run0) for k in range(n)]
    with open("../captures/requal_sr.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["t_s", "ci", "ma", "mv"])
        for k in range(n):
            w.writerow([round(ts[k], 1)] + list(rows[k]))

    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    def rpm(ci):
        return (2e6 / (6 * ci) * 60 / POLE_PAIRS) if ci else 0

    fig, ax = plt.subplots(3, 1, figsize=(16, 10), sharex=True)
    ax[0].plot(ts, [rpm(r[0]) for r in rows], lw=1.1, color="#2b6cb0")
    ax[0].set_ylabel("mechanical RPM")
    ax[0].grid(alpha=0.3)
    ax[1].plot(ts, [r[1] / 1000 for r in rows], lw=1.1, color="#c05621")
    ax[1].set_ylabel("current (A)")
    ax[1].grid(alpha=0.3)
    ax[2].plot(ts, [r[2] / 1000 for r in rows], lw=1.1, color="#276749")
    ax[2].set_ylabel("vbat (V)")
    ax[2].set_xlabel("time (s)")
    ax[2].grid(alpha=0.3)
    for t_m, lbl in marks:
        for a in ax:
            a.axvline(t_m, color="#888", lw=0.6, ls="--", alpha=0.6)
        ax[0].text(t_m, ax[0].get_ylim()[1], lbl, fontsize=7,
                   rotation=90, va="top")
    ax[0].set_title("rm32 battery re-qual — onboard recorder "
                    f"(0.5s samples, poll-free run; n={n})")
    fig.tight_layout()
    fig.savefig("../captures/requal_charts.png", dpi=110)
    print("wrote captures/requal_sr.csv + captures/requal_charts.png")
    for s in segs:
        print(f"  {s}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
