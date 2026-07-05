#!/usr/bin/env python3
"""PEACOCK: rinz-style 6-sector plots from a MAGPIE window-record capture.

Reads a capture produced by `uart_stream.py --out`, renders one figure:
  - 6 panels (one per sector, rinz layout): first-valid-ZC offset within
    the window over time (scatter), window length (top line) and the
    half-window time gate (dashed) for reference.
  - Per-panel annotation: zc-found %, mean +/- sd of zc_off, mean
    raw/valid edge counts.

Usage:
    python scripts/plot_windows.py captures/foo.bin [-o out.png]
"""

import argparse
import pathlib
import statistics

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

ap = argparse.ArgumentParser()
ap.add_argument("capture")
ap.add_argument("-o", "--out", default=None, help="output PNG (default: <capture>.png)")
ap.add_argument("--title", default=None)
args = ap.parse_args()

raw = pathlib.Path(args.capture).read_bytes()


def parse_frames(buf: bytes):
    frames = []
    i = 0
    while i + 16 <= len(buf):
        if buf[i] == 0x5A and buf[i + 1] == 0xA5 and (buf[i + 3] & 0x0F) < 6:
            f = buf[i : i + 16]
            frames.append(
                dict(
                    seq=f[2],
                    zc_found=bool(f[3] & 0x80),
                    sector=f[3] & 0x0F,
                    start=int.from_bytes(f[4:8], "little"),
                    len_us=int.from_bytes(f[8:10], "little") * 10,
                    zc_off_us=int.from_bytes(f[10:12], "little"),
                    raw=int.from_bytes(f[12:14], "little"),
                    valid=int.from_bytes(f[14:16], "little"),
                )
            )
            i += 16
        else:
            i += 1
    return frames


frames = parse_frames(raw)
if not frames:
    raise SystemExit("no frames in capture")

t0 = frames[0]["start"]
for f in frames:
    # 10 us ticks -> seconds since capture start (u32 wrap-safe)
    f["t"] = ((f["start"] - t0) & 0xFFFFFFFF) * 10e-6

gaps = sum(1 for a, b in zip(frames, frames[1:]) if (a["seq"] + 1) % 256 != b["seq"])

fig, axes = plt.subplots(2, 3, figsize=(15, 8), sharex=True, sharey=True)
fig.suptitle(
    args.title
    or f"{args.capture} — {len(frames)} windows, {gaps} seq gaps",
    fontsize=13,
)

for s in range(6):
    ax = axes[s // 3][s % 3]
    fs = [f for f in frames if f["sector"] == s]
    if not fs:
        ax.set_title(f"sector {s} — no data")
        continue

    t_zc = [f["t"] for f in fs if f["zc_found"]]
    zc = [f["zc_off_us"] for f in fs if f["zc_found"]]
    t_miss = [f["t"] for f in fs if not f["zc_found"]]

    ax.plot([f["t"] for f in fs], [f["len_us"] for f in fs], color="0.6", lw=0.8, label="window len")
    ax.plot(
        [f["t"] for f in fs],
        [f["len_us"] / 2 for f in fs],
        color="0.6",
        lw=0.8,
        ls="--",
        label="half-window gate",
    )
    ax.scatter(t_zc, zc, s=2, color="tab:blue", label="first valid ZC")
    if t_miss:
        ax.scatter(t_miss, [0] * len(t_miss), s=6, color="tab:red", marker="x", label="no ZC")

    zc_pct = 100 * len(zc) / len(fs)
    stats = f"zc {zc_pct:.0f}%"
    if len(zc) >= 2:
        stats += f"  off {statistics.mean(zc):.0f}±{statistics.stdev(zc):.0f}µs"
    stats += (
        f"\nraw {statistics.mean([f['raw'] for f in fs]):.0f}"
        f"  valid {statistics.mean([f['valid'] for f in fs]):.0f}"
    )
    ax.set_title(f"sector {s}   {stats}", fontsize=9)
    ax.grid(True, alpha=0.3)
    if s == 0:
        ax.legend(fontsize=7, loc="lower right")
    if s >= 3:
        ax.set_xlabel("time (s)")
    if s % 3 == 0:
        ax.set_ylabel("µs from window start")

fig.tight_layout()
out = args.out or str(pathlib.Path(args.capture).with_suffix(".png"))
fig.savefig(out, dpi=110)
print(f"wrote {out}")
