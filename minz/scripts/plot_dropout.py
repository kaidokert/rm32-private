#!/usr/bin/env python3
"""Dropout forensics: high-rate time-series of the final seconds of a
run's LAST capture — speed, per-window supply-voltage minimum (v4
frames, 48 kHz-pumped firmware aggregation), current, and qZC hits —
so a kill's cause is VISIBLE, not inferred. Standing bench practice:
after every test, show the lock map AND this.

Usage:
    python scripts/plot_dropout.py --tag board2h [--secs 4]
"""

import argparse
import glob
import pathlib
import re
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

from magpie import parse_frames, raw_to_ma, vbat_raw_to_v

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--tag", required=True)
ap.add_argument("--secs", type=float, default=4.0, help="tail length to plot")
ap.add_argument("-o", "--out", default=None)
args = ap.parse_args()

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"

# Last capture bin of the run = the rung where it died (or the last
# completed one). The session log supplies the kill line for a title.
bins = sorted(
    glob.glob(str(capdir / f"{args.tag}_a*.bin"))
    + glob.glob(str(capdir / f"{args.tag}_t*.bin")),
    key=lambda p: pathlib.Path(p).stat().st_mtime,
)
if not bins:
    sys.exit("no captures for tag")
# Walk back from the newest: a kill can land BETWEEN captures,
# leaving the newest transit bin empty — the last non-empty bin is
# then the final streamed evidence before the drop.
last, frames = None, []
for cand in reversed(bins):
    frames = parse_frames(pathlib.Path(cand).read_bytes())
    if len(frames) >= 20:
        last = cand
        break
    print(f"note: {pathlib.Path(cand).name}: only {len(frames)} frames, "
          "skipping back (kill likely landed between captures)")
if last is None:
    sys.exit("no bin with enough frames")

kill = ""
log = capdir / f"{args.tag}_session.log"
if log.exists():
    txt = log.read_bytes().decode("ascii", errors="replace")
    for kw in ("VBAT SAG KILL", "OVERCURRENT TRIP", "ZC-STARVED", "CL DESYNC"):
        i = txt.rfind(kw)
        if i >= 0:
            kill = txt[i : txt.find("\r", i)][:110]
            break

t0 = frames[0]["start"]
t = [(f["start"] - t0) * 10 / 1e6 for f in frames]  # seconds
t_end = t[-1]
lo = next(i for i, x in enumerate(t) if x >= t_end - args.secs)
fr = frames[lo:]
t = t[lo:]

fig, axes = plt.subplots(3, 1, figsize=(13, 8), sharex=True)
name = pathlib.Path(last).name
fig.suptitle(f"dropout tail — {name}\n{kill}", fontsize=10)

axes[0].plot(t, [1e6 / (6 * f["len_us"]) if f["len_us"] else 0 for f in fr],
             lw=0.8, color="tab:blue")
axes[0].set_ylabel("f_e (Hz)")

vb = [(x, vbat_raw_to_v(f["vbat_raw"])) for x, f in zip(t, fr) if f.get("vbat_raw")]
if vb:
    axes[1].plot([p[0] for p in vb], [p[1] for p in vb], lw=0.8, color="tab:red")
    axes[1].set_ylabel("vbat window-min (V)")
else:
    axes[1].text(0.5, 0.5, "no v4 vbat in this capture", ha="center",
                 transform=axes[1].transAxes)

axes[2].plot(t, [raw_to_ma(f["i_avg"]) for f in fr], lw=0.8, color="tab:purple",
             label="i_avg")
axes[2].plot(t, [raw_to_ma(f["i_max"]) for f in fr], lw=0.5, alpha=0.5,
             color="tab:purple", label="i_max")
noz = [x for x, f in zip(t, fr) if f["qzc_off_us"] == 0xFFFF]
if noz:
    for x in noz:
        axes[2].axvline(x, color="orange", alpha=0.15, lw=0.8)
axes[2].set_ylabel("current (mA)")
axes[2].set_xlabel("time (s) — orange bands = windows with NO qualified ZC")
axes[2].legend(fontsize=8)
for ax in axes:
    ax.grid(True, alpha=0.3)

fig.tight_layout()
out = args.out or str(capdir / f"{args.tag}_dropout.png")
fig.savefig(out, dpi=110)
print(f"wrote {out}")
