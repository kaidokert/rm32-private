"""Side-by-side setpoint step response: minz (MAGPIE ladder capture)
vs AM32 (ZCTRACE csv from zctrace_capture.py) on shared axes.

Each panel: per-window/per-commutation electrical frequency vs time.
The goal metric is visual: rpm follows throttle in steps of the same
character — no pulsing beyond what AM32 shows.

Usage:
  python scripts/plot_stepresp.py --minz captures/ladder_cap_X.bin \
      --am32 captures/lowspeed_trace.csv [--out captures/stepresp.png]
"""

import argparse
import csv
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

sys.path.insert(0, "scripts")
import magpie


def minz_series(path):
    frames = magpie.parse_frames(open(path, "rb").read())
    ts, hz = [], []
    t0 = None
    for f in frames:
        wlen = f.get("len_us") or 0
        if not wlen or wlen > 20000:
            continue
        if t0 is None:
            t0 = f["start"]
        t = (f["start"] - t0) * 10e-6
        if t < 0:
            continue
        ts.append(t)
        hz.append(1e6 / (6 * wlen))
    return ts, hz


def am32_series(path):
    # One record per commutation; zt_us is the ZC-to-ZC interval, so
    # cumulative zt reconstructs the time axis.
    ts, hz, acc = [], [], 0.0
    for row in csv.DictReader(open(path)):
        zt = float(row["zt_us"])
        if not (0 < zt < 20000):
            continue
        acc += zt * 1e-6
        ts.append(acc)
        hz.append(1e6 / (6 * zt))
    return ts, hz


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--minz", required=True)
    ap.add_argument("--am32", required=True)
    ap.add_argument("--title", default="setpoint step response - minz vs AM32")
    ap.add_argument("--out", default="captures/stepresp.png")
    a = ap.parse_args()

    mts, mhz = minz_series(a.minz)
    ats, ahz = am32_series(a.am32)
    print(f"minz: {len(mts)} windows over {mts[-1]:.1f}s, "
          f"{min(mhz):.0f}-{max(mhz):.0f} Hz")
    print(f"am32: {len(ats)} records over {ats[-1]:.1f}s, "
          f"{min(ahz):.0f}-{max(ahz):.0f} Hz")

    fig, axes = plt.subplots(2, 1, figsize=(14, 9))
    axes[0].plot(mts, mhz, ".", ms=1.5, color="tab:red", label="minz")
    axes[0].set_ylabel("electrical Hz")
    axes[0].set_title(f"{a.title} — minz")
    axes[0].legend()
    axes[1].plot(ats, ahz, ".", ms=1.5, color="tab:green", label="AM32")
    axes[1].set_ylabel("electrical Hz")
    axes[1].set_xlabel("time (s)")
    axes[1].set_title("AM32 (ZCTRACE, same bench)")
    axes[1].legend()
    plt.tight_layout()
    plt.savefig(a.out, dpi=110)
    print("wrote", a.out)


if __name__ == "__main__":
    main()
