#!/usr/bin/env python3
"""WAXWING: capture + render a waveform burst from motor_tester2.

Sends `j`, reads the rinz-format dump (`cdump: N frames b85 4 channels
(ch9 ch10 ch8 ch99) ...` + Ascii85 payload + `end`), and renders a
latest_zc-style figure:

  - panel A / B: true phase-voltage waveforms (PA4 / PA5 dividers),
    float windows shaded, virtual neutral dashed, sign-change (o) and
    least-squares linfit (^) ZC markers, comparator edges (x), current
    on the right axis.
  - panel C: PB7 has no ADC route — the comparator bit IS the C
    channel. Rendered as a step trace in C's float windows.

Frame = one PWM cycle (24 kHz), sampled at CNT=250 (~3.1 µs into the
ON window). Channels: ch9=A, ch10=B, ch8=current, ch99=status
(bit0 comp value, bits1-3 sector).

Keep the MAGPIE stream (`g`) OFF while capturing — binary frames would
interleave with the dump text.

Usage:
    python scripts/waxwing.py [--frames 600] [--tag waxwing]
    python scripts/waxwing.py --infile captures/waxwing_x.txt   # re-render
"""

import argparse
import base64
import pathlib
import struct
import time

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--frames", type=int, default=600, help="most-recent frames to plot")
ap.add_argument("--tag", default="waxwing")
ap.add_argument("--infile", default=None, help="re-render an existing capture")
ap.add_argument("--smooth", type=int, default=3)
args = ap.parse_args()

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
capdir.mkdir(exist_ok=True)

# ------------------------------------------------------------------
# Capture
# ------------------------------------------------------------------
if args.infile:
    # Ladder captures interleave binary MAGPIE frames with the text
    # dumps; latin-1 maps every byte so the cdump lines survive.
    text = pathlib.Path(args.infile).read_text(encoding="latin-1")
    stem = pathlib.Path(args.infile).stem
else:
    import serial

    with serial.Serial(args.port, args.baud, timeout=0.1) as p:
        p.reset_input_buffer()
        p.write(b"j")
        buf = bytearray()
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            buf += p.read(65536)
            if b"\nend" in buf or b"\rend" in buf:
                break
    text = buf.decode("ascii", errors="replace")
    stem = f"{args.tag}_{time.strftime('%H%M%S')}"
    (capdir / f"{stem}.txt").write_text(text)
    print(f"saved {len(text)} chars to captures/{stem}.txt")

# ------------------------------------------------------------------
# Parse (rinz cdump conventions)
# ------------------------------------------------------------------
lines = text.splitlines()
hdr_i = next(i for i, l in enumerate(lines) if l.lstrip().startswith("cdump:"))
header = lines[hdr_i]
n_frames = int(header.split()[1])
sample_hz = float(header.split("sample_hz=")[1].split()[0])
body = []
for l in lines[hdr_i + 1 :]:
    if l.strip() == "end":
        break
    body.append(l.strip())
raw = base64.a85decode("".join(body).encode("ascii"))
vals = struct.unpack(f"<{len(raw) // 2}H", raw[: len(raw) // 2 * 2])
n = len(vals) // 4
W = np.array(vals[: n * 4], dtype=float).reshape(n, 4)
ST = W[:, 3].astype(int)
sector = (ST >> 1) & 0x7
comp = ST & 1
print(f"parsed {n} frames @ {sample_hz:.0f} Hz")

A = W[:, 0].copy()
B = W[:, 1].copy()
I = W[:, 2].copy()

# Most-recent slice
if args.frames and n > args.frames:
    sl = slice(n - args.frames, n)
    A, B, I, sector, comp = A[sl], B[sl], I[sl], sector[sl], comp[sl]
    n = args.frames

t_ms = np.arange(n) / sample_hz * 1e3


def smooth(x, w):
    if w <= 1:
        return x
    k = np.ones(w) / w
    return np.convolve(x, k, mode="same")


As, Bs = smooth(A, args.smooth), smooth(B, args.smooth)

# ------------------------------------------------------------------
# Sector geometry + neutral
# ------------------------------------------------------------------
# Textbook six-step: (high, low, float) per sector; A/B measured,
# C estimated as vbus (high) or 0 (low) at the mid-ON sample instant.
# sector: 0:(A,B,C) 1:(A,C,B) 2:(B,C,A) 3:(B,A,C) 4:(C,A,B) 5:(C,B,A)
FLOATS = {0: "C", 1: "B", 2: "A", 3: "C", 4: "B", 5: "A"}
vbus = float(np.percentile(np.maximum(A, B), 98))

neutral = np.zeros(n)
for i in range(n):
    s = sector[i]
    if s in (0, 3):  # A,B driven
        neutral[i] = (As[i] + Bs[i]) / 2
    elif s == 1:  # A high, C low; B floats
        neutral[i] = (As[i] + 0.0) / 2
    elif s == 4:  # C high, A low; B floats
        neutral[i] = (vbus + As[i]) / 2
    elif s == 2:  # B high, C low; A floats
        neutral[i] = (Bs[i] + 0.0) / 2
    else:  # 5: C high, B low; A floats
        neutral[i] = (vbus + Bs[i]) / 2

# Sector windows: list of (start, end, sector)
windows = []
w0 = 0
for i in range(1, n):
    if sector[i] != sector[i - 1]:
        windows.append((w0, i, sector[i - 1]))
        w0 = i
windows.append((w0, n, sector[n - 1]))

BLANK = 2  # frames skipped at window start for fits (mux/commutation)


def window_zcs(trace, win_sectors):
    """(sign-change idx, linfit crossing idx, window) per float window."""
    out = []
    for s0, s1, sec in windows:
        if sec not in win_sectors or s1 - s0 < BLANK + 3:
            continue
        e = trace[s0 + BLANK : s1] - neutral[s0 + BLANK : s1]
        # sign change with 2-frame confirmation
        zc_sign = None
        for k in range(1, len(e) - 1):
            if e[k - 1] * e[k] <= 0 and e[k] * e[k + 1] >= 0 and e[k - 1] != e[k]:
                zc_sign = s0 + BLANK + k
                break
        # least-squares line through the window, crossing of zero
        x = np.arange(len(e))
        if len(e) >= 3 and np.ptp(e) > 0:
            m, b = np.polyfit(x, e, 1)
            if m != 0:
                xz = -b / m
                if -1 <= xz <= len(e) + 1:
                    out.append((zc_sign, s0 + BLANK + xz, (s0, s1, sec)))
                    continue
        out.append((zc_sign, None, (s0, s1, sec)))
    return out


# ------------------------------------------------------------------
# Render
# ------------------------------------------------------------------
fig, axes = plt.subplots(3, 1, figsize=(15, 10), sharex=True)
colors = {"A": "tab:green", "B": "tab:blue", "C": "tab:red"}
shade = {"A": (2, 5), "B": (1, 4), "C": (0, 3)}
cur_color = "#7d5ba6"

# f estimate from mean full-sector length (interior windows only)
full = [s1 - s0 for s0, s1, _ in windows[1:-1]]
f_est = sample_hz / (np.mean(full) * 6) if full else 0
i_ma = np.mean(I) * 3300 / 4095 / 30 * 1000

fig.suptitle(
    f"{stem}  |  f≈{f_est:.0f} Hz  {np.mean(full) if full else 0:.1f} frames/sector  "
    f"i≈{i_ma:.0f} mA  |  {n} frames @ {sample_hz / 1000:.1f} kframes/s",
    fontsize=12,
)

for ax, phase in zip(axes, "ABC"):
    color = colors[phase]
    # sector shading: this phase's float windows tinted, others gray grid
    for s0, s1, sec in windows:
        if sec in shade[phase]:
            ax.axvspan(t_ms[s0], t_ms[min(s1, n - 1)], color=color, alpha=0.08)
        ax.axvline(t_ms[s0], color="0.85", lw=0.6, zorder=0)
        if s1 - s0 > 6:
            ax.text(
                t_ms[(s0 + s1) // 2], 0.97, str(sec), transform=ax.get_xaxis_transform(),
                ha="center", va="top", fontsize=7,
                color=color if sec in shade[phase] else "0.6",
            )

    if phase in ("A", "B"):
        trace = As if phase == "A" else Bs
        ax.plot(t_ms, trace, color=color, lw=1.0, label=f"{phase} (smoothed {args.smooth})")
        ax.plot(t_ms, neutral, color="0.4", lw=0.8, ls="--", label="virtual neutral")
        for zc_sign, zc_fit, (s0, s1, sec) in window_zcs(trace, shade[phase]):
            if zc_sign is not None:
                ax.plot(t_ms[zc_sign], trace[zc_sign], "o", color="tab:orange", ms=5, mfc="none")
            if zc_fit is not None:
                tz = np.interp(zc_fit, np.arange(n), t_ms)
                yz = np.interp(zc_fit, np.arange(n), neutral)
                ax.plot(tz, yz, "^", color=color, ms=7, mfc="none")
        # comparator edges inside this phase's float windows (auto-mux:
        # the comp bit tracks each sector's floating phase)
        for s0, s1, sec in windows:
            if sec not in shade[phase]:
                continue
            for k in range(max(s0 + 1, 1), s1):
                if comp[k] != comp[k - 1]:
                    ax.plot(t_ms[k], neutral[k], "x", color="magenta", ms=6)
        ax2 = ax.twinx()
        ax2.plot(t_ms, I, color=cur_color, lw=0.7, alpha=0.6)
        ax2.set_ylabel("I (counts)", color=cur_color, fontsize=8)
        ax2.tick_params(labelsize=7, colors=cur_color)
    else:
        # C: comparator bit as the channel (PB7 has no ADC route)
        ax.plot(t_ms, comp * vbus * 0.8, color=color, lw=0.9, drawstyle="steps-post",
                label="COMP2 value (C float windows = phase C vs neutral)")
        ax.plot(t_ms, neutral, color="0.4", lw=0.8, ls="--")

    ax.set_ylabel(f"{phase} (12-bit)")
    ax.legend(fontsize=8, loc="upper right")
    ax.grid(True, alpha=0.25)

axes[-1].set_xlabel(f"time (ms) @ {sample_hz / 1000:.1f} kframes/s")
fig.text(
    0.5, 0.005,
    "ZC markers:  o sign-change   ^ least-squares linfit crossing   x comparator edge",
    ha="center", fontsize=8,
)
fig.tight_layout()
out = capdir / f"{stem}.png"
fig.savefig(out, dpi=110)
print(f"wrote {out}")
