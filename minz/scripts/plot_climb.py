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
    ts, hz, vb, ratio, sec = [], [], [], [], []
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
        q = f.get("qzc_off_us", 0xFFFF)
        ratio.append(q / wlen_us if q != 0xFFFF and wlen_us else None)
        sec.append(f["sector"])
    return ts, hz, vb, ratio, sec


def slope_split(ts, hz, ratio, win=25):
    """Smoothed df/dt sign per window; returns (rising_ratios,
    falling_ratios) for the gate-echo discriminator."""
    rising, falling = [], []
    n = len(ts)
    for k in range(n):
        if ratio[k] is None:
            continue
        lo = max(0, k - win)
        hi = min(n - 1, k + win)
        if hi <= lo:
            continue
        slope = (hz[hi] - hz[lo]) / max(ts[hi] - ts[lo], 1e-6)
        (rising if slope > 200.0 else falling if slope < -200.0
         else []).append(ratio[k])
    return rising, falling


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("--am32", default=None)
    ap.add_argument("--tmax", type=float, default=None)
    ap.add_argument("--out", default="captures/climb_wobble.png")
    a = ap.parse_args()

    ts, hz, vb, ratio, sec = minz_series(a.capture)
    if not ts:
        raise SystemExit(f"no MAGPIE frames in {a.capture} "
                         "(was the stream on during the run?)")
    if a.tmax:
        keep = [k for k, t in enumerate(ts) if t <= a.tmax]
        ts = [ts[k] for k in keep]
        hz = [hz[k] for k in keep]
        vb = [vb[k] for k in keep]
        ratio = [ratio[k] for k in keep]
        sec = [sec[k] for k in keep]
    print(f"{len(ts)} windows over {ts[-1]:.1f}s, "
          f"{min(hz):.0f}-{max(hz):.0f} Hz")

    # Per-sector window-length asymmetry (the two-band structure).
    per = {s: [] for s in range(6)}
    for h, s in zip(hz, sec):
        per[s].append(h)
    line = " ".join(
        f"s{s}:{(sum(v)/len(v)):.0f}Hz(n={len(v)})" if v else f"s{s}:-"
        for s, v in per.items())
    print("per-sector mean f_e:", line)
    # Timing skeleton: per-sector window length + qZC offset (µs).
    plen = {s: [] for s in range(6)}
    pqzc = {s: [] for s in range(6)}
    for h, s, r in zip(hz, sec, ratio):
        wl = 1e6 / (6 * h)
        plen[s].append(wl)
        if r is not None:
            pqzc[s].append(r * wl)
    print("per-sector wl/qzc us:", " ".join(
        f"s{s}:{sum(plen[s])/max(len(plen[s]),1):.0f}/"
        f"{sum(pqzc[s])/max(len(pqzc[s]),1):.0f}"
        for s in range(6)))

    # Gate-echo discriminator: during surge (rising f), do accepted
    # qZCs hug the 30% gate (self-referential accept) or sit where
    # real BEMF ZCs live (~mid-window)?
    rising, falling = slope_split(ts, hz, ratio)

    def stats(v):
        if not v:
            return "n=0"
        v = sorted(v)
        return (f"n={len(v)} mean={sum(v)/len(v):.3f} "
                f"p10={v[len(v)//10]:.3f} med={v[len(v)//2]:.3f} "
                f"p90={v[9*len(v)//10]:.3f}")

    print("qzc_off/window RISING :", stats(rising))
    print("qzc_off/window FALLING:", stats(falling))

    fig, axes = plt.subplots(3, 1, figsize=(14, 12), sharex=True,
                             squeeze=False)
    ax = axes[0][0]
    ax.plot(ts, hz, ".", ms=2, color="tab:red", label="minz per-window f_e")
    ax.set_ylabel("electrical Hz")
    ax.set_title("climb wobble - per-window electrical frequency vs time")
    ax.legend()
    rts = [t for t, r in zip(ts, ratio) if r is not None]
    rvs = [r for r in ratio if r is not None]
    axes[1][0].plot(rts, rvs, ".", ms=2, color="tab:purple",
                    label="qzc_off / window")
    axes[1][0].axhline(0.30, color="gray", lw=1, ls="--", label="gate (30%)")
    axes[1][0].set_ylabel("qZC position in window")
    axes[1][0].set_ylim(0, 1)
    axes[1][0].legend()
    vts = [t for t, v in zip(ts, vb) if v]
    vvs = [magpie.vbat_raw_to_v(v) for v in vb if v]
    axes[2][0].plot(vts, vvs, ".", ms=2, color="tab:blue", label="vbat")
    axes[2][0].set_ylabel("bus V")
    axes[2][0].legend()
    axes[-1][0].set_xlabel("time (s)")
    plt.tight_layout()
    plt.savefig(a.out, dpi=110)
    print("wrote", a.out)


if __name__ == "__main__":
    main()
