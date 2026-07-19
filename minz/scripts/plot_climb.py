"""Climb wobble time-series: electrical Hz vs time from a ladder
serial capture (MAGPIE frames interleaved with ASCII), optionally
overlaid with an AM32 ZCTRACE capture ramp.

The per-rung ladder log samples once per 500 ms and hides the
audible walk-up-and-down; this plots EVERY streamed float window
(1/5 decimation above ~925 Hz), so reseed dips, wobble, and
recovery ramps are visible at window resolution.

Usage:
  python scripts/plot_climb.py captures/ladder_cap_TAG_r0.bin \
         [--am32 captures/am32_ramp.bin] [--out captures/climb_wobble.png]
"""

import argparse
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

sys.path.insert(0, "scripts")
import magpie


def minz_series(path):
    buf = open(path, "rb").read()
    frames = magpie.parse_frames(buf)
    ts, hz, vb = [], [], []
    t0 = None
    for f in frames:
        # start ticks are 10 us; window len gives the sector period
        wlen_us = f.get("len_us") or f.get("len_ticks", 0) * 10
        if not wlen_us or wlen_us > 20000:
            continue
        if t0 is None:
            t0 = f["start"]
        t = (f["start"] - t0) * 10e-6
        if t < 0:  # tick wrap in a long capture
            continue
        ts.append(t)
        hz.append(1e6 / (6 * wlen_us))
        vb.append(f.get("vbat_raw"))
    return ts, hz, vb


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("--am32", default=None)
    ap.add_argument("--out", default="captures/climb_wobble.png")
    a = ap.parse_args()

    ts, hz, vb = minz_series(a.capture)
    if not ts:
        raise SystemExit(f"no MAGPIE frames in {a.capture} "
                         "(was the stream on during the run?)")
    print(f"{len(ts)} windows over {ts[-1]:.1f}s, "
          f"{min(hz):.0f}-{max(hz):.0f} Hz")

    n = 2 if any(v for v in vb) else 1
    fig, axes = plt.subplots(n, 1, figsize=(14, 4 * n + 2), sharex=True,
                             squeeze=False)
    ax = axes[0][0]
    ax.plot(ts, hz, ".", ms=2, color="tab:red", label="minz per-window f_e")
    ax.set_ylabel("electrical Hz")
    ax.set_title("climb wobble - per-window electrical frequency vs time")
    ax.legend()
    if n == 2:
        vts = [t for t, v in zip(ts, vb) if v]
        vvs = [magpie.vbat_raw_to_v(v) for v in vb if v]
        axes[1][0].plot(vts, vvs, ".", ms=2, color="tab:blue", label="vbat")
        axes[1][0].set_ylabel("bus V")
        axes[1][0].legend()
    axes[-1][0].set_xlabel("time (s)")
    plt.tight_layout()
    plt.savefig(a.out, dpi=110)
    print("wrote", a.out)


if __name__ == "__main__":
    main()
