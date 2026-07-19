"""Sawtooth trace: the SLOW control variables through a throttle
step, from a probe capture containing q-lines + MAGPIE frames.

q-line: "q <t10us> <target%> <applied%> <est_us> <stiff_us> <adv_deg>
         <trim_even> <trim_odd>"

Panels: per-window f_e, duty (target vs applied), interval estimates
(fast vs stiff, as equivalent Hz), advance, parity trims.

Usage: python scripts/plot_qlog.py captures/ab_TAG_A.bin [B.bin ...]
       [--out captures/qlog.png]
"""

import argparse
import re
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

sys.path.insert(0, "scripts")
import magpie

QLINE = re.compile(
    rb"q (\d+) (\d+) (\d+) (\d+) (\d+) (-?\d+) (-?\d+) (-?\d+)"
    rb"(?: (\d+) (\d+))?")


def parse(paths):
    buf = b"".join(open(p, "rb").read() for p in paths)
    q = [tuple(int(g) if g is not None else 0 for g in m.groups())
         for m in QLINE.finditer(buf)]
    frames = magpie.parse_frames(buf)
    return q, frames


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("captures", nargs="+")
    ap.add_argument("--out", default="captures/qlog.png")
    a = ap.parse_args()
    q, frames = parse(a.captures)
    if not q:
        raise SystemExit("no q-lines (was Q pressed?)")
    t0 = q[0][0]
    qt = [(r[0] - t0) * 1e-5 for r in q]
    print(f"{len(q)} q-lines over {qt[-1]:.1f}s, {len(frames)} frames")

    fts, fhz = [], []
    f0 = None
    for f in frames:
        wl = f.get("len_us") or 0
        if not wl or wl > 20000:
            continue
        if f0 is None:
            f0 = f["start"]
        fts.append((f["start"] - f0) * 1e-5)
        fhz.append(1e6 / (6 * wl))

    fig, axes = plt.subplots(5, 1, figsize=(14, 14), sharex=True)
    axes[0].plot(fts, fhz, ".", ms=1.5, color="tab:red")
    axes[0].set_ylabel("f_e (Hz)")
    axes[0].set_title("sawtooth trace - slow control variables through the step")
    axes[1].plot(qt, [r[1] for r in q], "-", label="target %")
    axes[1].plot(qt, [r[2] for r in q], "-", label="applied %")
    axes[1].set_ylabel("duty %")
    axes[1].legend()
    est_hz = [1e6 / (6 * r[3]) if r[3] else 0 for r in q]
    stiff_hz = [1e6 / (6 * r[4]) if r[4] else 0 for r in q]
    axes[2].plot(qt, est_hz, "-", label="fast est (Hz)")
    axes[2].plot(qt, stiff_hz, "-", label="stiff est (Hz)")
    axes[2].set_ylabel("est (Hz)")
    axes[2].legend()
    axes[3].plot(qt, [r[5] for r in q], "-", label="advance deg")
    axes[3].set_ylabel("adv (deg)")
    axes[3].legend()
    axes[4].plot(qt, [r[6] for r in q], "-", label="trim even us")
    axes[4].plot(qt, [r[7] for r in q], "-", label="trim odd us")
    axes[4].set_ylabel("trim (us)")
    axes[4].set_xlabel("time (s)")
    axes[4].legend()
    plt.tight_layout()
    plt.savefig(a.out, dpi=110)
    print("wrote", a.out)


if __name__ == "__main__":
    main()
