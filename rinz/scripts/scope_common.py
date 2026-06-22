#!/usr/bin/env python3
"""Shared helpers for rinz scope UART captures and PlotJuggler replay."""

from __future__ import annotations

from dataclasses import dataclass, field
import base64
import json
import math
import re
import socket
import statistics
import struct
import time
from collections import deque
from pathlib import Path
from typing import Iterable

import matplotlib
import numpy as np

matplotlib.use("Agg")
import matplotlib.pyplot as plt

BAUD = 115200
AMP_START_TENTHS = 80
FREQ_START_HZ = 60
FREQ_STEP_HZ = 10
SAMPLE_HZ = 20000
FRAME_HZ = 20000
FULL_SCALE = 255
VREF = 3.3
CHANNEL_PLOT_OFFSET = 4
LOWPASS_WINDOW = 9
DEFAULT_LABELS = ["ch17", "ch5", "ch14"]
# Plot panels top-to-bottom: phase name, capture channel index, ADC label.
PHASE_PLOT_ORDER = [("A", 0, "ch17"), ("B", 1, "ch5"), ("C", 2, "ch14")]


@dataclass
class Capture:
    """One parsed scope dump."""

    channels: list[list[int]]
    expected: int | None
    labels: list[str]
    sample_hz: float
    text: str = ""
    debug: dict[str, str] = field(default_factory=dict)
    regs: list[str] = field(default_factory=list)
    captured_at: float = field(default_factory=time.time)
    full_scale: int = FULL_SCALE
    # True for scope2 dual peak+valley captures: frames alternate a valley scan
    # (neutral ~Vbus/2, bipolar BEMF) and a peak scan (neutral ~GND, clamped BEMF).
    # The raw frame-to-frame signal zigzags, so deinterleave() before any analysis
    # that assumes a coherent stream (lowpass, analyze_zero_crossings, slope fits).
    interleaved: bool = False

    @property
    def frames(self) -> int:
        return len(self.channels[0]) if self.channels else 0

    @property
    def stats(self) -> dict[str, float | int | None]:
        flat = [v for ch in self.channels for v in ch]
        if not flat:
            return {"min": None, "max": None, "mean": None, "sat": 0}
        return {
            "min": min(flat),
            "max": max(flat),
            "mean": sum(flat) / len(flat),
            "sat": sum(1 for v in flat if v >= self.full_scale),
        }


def read_available(ser, idle_s: float = 0.15, max_s: float = 2.0) -> str:
    """Read until the UART has been idle for idle_s or max_s expires."""
    deadline = time.monotonic() + max_s
    idle_deadline = time.monotonic() + idle_s
    chunks = []

    while time.monotonic() < deadline:
        n = ser.in_waiting
        if n:
            chunks.append(ser.read(n))
            idle_deadline = time.monotonic() + idle_s
        elif time.monotonic() >= idle_deadline:
            break
        else:
            time.sleep(0.01)

    return b"".join(chunks).decode("ascii", errors="replace")


def send_key(ser, key: str, delay_s: float = 0.04) -> str:
    ser.write(key.encode("ascii"))
    ser.flush()
    time.sleep(delay_s)
    return read_available(ser, idle_s=0.04, max_s=0.4)


def send_keys(ser, key: str, count: int) -> str:
    out = []
    for _ in range(count):
        out.append(send_key(ser, key))
    return "".join(out)


def ramp_amplitude(ser, target_percent: float) -> str:
    target = int(round(target_percent * 10))
    delta = target - AMP_START_TENTHS
    out = []

    full_steps, tenths = divmod(abs(delta), 10)
    if delta >= 0:
        out.append(send_keys(ser, "a", full_steps))
        out.append(send_keys(ser, "+", tenths))
    else:
        out.append(send_keys(ser, "z", full_steps))
        out.append(send_keys(ser, "-", tenths))

    return "".join(out)


def ramp_frequency(ser, target_hz: int) -> str:
    delta = target_hz - FREQ_START_HZ
    steps = abs(delta) // FREQ_STEP_HZ
    return send_keys(ser, "f" if delta >= 0 else "v", steps)


def capture_dump(ser, timeout_s: float = 10.0) -> str:
    ser.write(b"d")
    ser.flush()

    deadline = time.monotonic() + timeout_s
    chunks = []
    while time.monotonic() < deadline:
        n = ser.in_waiting
        if n:
            chunk = ser.read(n)
            chunks.append(chunk)
            text = b"".join(chunks).decode("ascii", errors="replace")
            if "end" in text:
                return text
        else:
            time.sleep(0.01)

    text = b"".join(chunks).decode("ascii", errors="replace")
    raise TimeoutError(f"timed out waiting for dump end marker; captured {len(text)} chars")


def parse_key_values(line: str) -> dict[str, str]:
    out = {}
    for item in line.split():
        if "=" in item:
            key, value = item.split("=", 1)
            out[key.rstrip(":")] = value
    return out


def parse_dump(text: str) -> tuple[list[list[int]], int | None, list[str], float]:
    capture = parse_capture(text)
    return capture.channels, capture.expected, capture.labels, capture.sample_hz


def parse_capture(text: str) -> Capture:
    lines = text.splitlines()
    try:
        start = next(
            i
            for i, line in enumerate(lines)
            if re.match(r"c?dump\d*:", line.lstrip())
            or line.lstrip().startswith("CAP_BEGIN")
        )
    except StopIteration:
        raise ValueError("no dump header found in captured text")

    debug = {}
    regs = []
    for line in lines[:start]:
        stripped = line.strip()
        if stripped.startswith("debug:"):
            debug.update(parse_key_values(stripped))
        elif stripped.startswith("regs:"):
            regs.append(stripped)

    header = lines[start].lstrip()
    is_cap = header.startswith("CAP_BEGIN")
    # b85 = Ascii85-packed u16 payload (cdump:); otherwise hex. Either way the channel
    # list in the header ("... 7 channels (ch17 ch5 ch14 ch16 ch18 ch13 ch1, ...)") is
    # self-describing: channels 0-2 are the BEMF voltages, extras are currents/VBUS.
    is_b85 = "b85" in header
    # scope2 marks dual peak+valley dumps; frames alternate valley/peak (see Capture).
    is_interleaved = "interleaved" in header
    is_multi = bool(re.match(r"dump\d+:", header)) or header.startswith("cdump:") or is_cap
    hdr_chans = re.findall(r"ch\d+", header)
    m = re.search(r"c?dump\d*:\s*(\d+)", header)
    if m is None and is_cap:
        m = re.search(r"\bframes=(\d+)", header)
    expected = int(m.group(1)) if m else None
    sample_hz_match = re.search(r"(\d+(?:\.\d+)?)\s*Hz", header)
    if sample_hz_match is None and is_cap:
        sample_hz_match = re.search(r"\bsample_hz=(\d+(?:\.\d+)?)", header)
    header_sample_hz = float(sample_hz_match.group(1)) if sample_hz_match else None

    body = []
    for line in lines[start + 1 :]:
        # Whole-line "end" is the terminator; b85 payload may contain it as a
        # substring mid-line, so don't break on substring.
        if line.strip() == "end":
            break
        body.append(line)
        if "CAP_END" in line:
            break

    is_12bit = "12-bit" in header
    full_scale = 4095 if is_12bit else FULL_SCALE

    if is_b85:
        # Ascii85-packed u16 little-endian samples; a85decode ignores the newlines.
        raw = base64.a85decode("".join(body).encode("ascii"))
        vals = list(struct.unpack(f"<{len(raw) // 2}H", raw[: len(raw) // 2 * 2]))
    else:
        hex_pat = "[0-9a-fA-F]{4}" if is_12bit else "[0-9a-fA-F]{2}"
        blob = " ".join(body).replace("CAP_DATA", " ").replace("CAP_END", " ").replace("end", " ")
        vals = [int(tok, 16) for tok in blob.split() if re.fullmatch(hex_pat, tok)]

    if not is_multi:
        channels = [vals]
        labels = ["ch17"]
        sample_hz = header_sample_hz or SAMPLE_HZ
    else:
        n_chan = len(hdr_chans) if hdr_chans else 3
        usable = len(vals) - len(vals) % n_chan
        frames = vals[:usable]
        channels = [frames[i::n_chan] for i in range(n_chan)]
        # Channels 0-2 keep the BEMF voltage labels (ZC analysis indexes them via
        # PHASE_TO_CHANNEL); extras carry their raw ch-name from the header.
        labels = (DEFAULT_LABELS + hdr_chans[len(DEFAULT_LABELS):])[:n_chan]
        sample_hz = header_sample_hz or FRAME_HZ

    return Capture(
        channels=channels,
        expected=expected,
        labels=labels,
        sample_hz=sample_hz,
        text=text,
        debug=debug,
        regs=regs,
        full_scale=full_scale,
        interleaved=is_interleaved,
    )


def split_complete_dumps(buffer: str) -> tuple[list[str], str]:
    """Split streamed UART text on the 'end' terminator.

    Returns (segments, remainder): each segment is one complete dump
    (debug + regs + dump3 + hex), parseable by parse_capture; remainder is the
    trailing partial dump still being received. The firmware emits 'end' only as
    the dump terminator, so splitting on it isolates whole dumps.
    """
    segments = []
    # Terminator is "end" on its own line. b85 (Ascii85) payload can contain "end" as
    # a mid-line substring, so anchor the split to a preceding CR/LF.
    term = re.compile(r"[\r\n]end\b")
    while True:
        m = term.search(buffer)
        if not m:
            break
        seg, buffer = buffer[: m.start()], buffer[m.end() :]
        if re.search(r"c?dump\d*:", seg):
            segments.append(seg)
    return segments, buffer


def lowpass_channels(channels: list[list[int]], window: int = LOWPASS_WINDOW) -> list[list[float]]:
    """Centered moving-average view. Does not modify the captured samples."""
    window = max(1, int(window))
    if window <= 1:
        return [[float(v) for v in vals] for vals in channels]

    half = window // 2
    filtered = []
    for vals in channels:
        prefix = [0.0]
        for v in vals:
            prefix.append(prefix[-1] + v)

        out = []
        for i in range(len(vals)):
            start = max(0, i - half)
            end = min(len(vals), i + half + 1)
            out.append((prefix[end] - prefix[start]) / (end - start))
        filtered.append(out)

    return filtered


def auto_snapshot_ylim(filtered: list[list[float]]) -> tuple[float, float]:
    """Derive shared Y limits from filtered data with 5% swing margin, rounded to 5."""
    flat = [v for ch in filtered for v in ch]
    if not flat:
        return (0.0, 255.0)

    lo = min(flat)
    hi = max(flat)
    delta = hi - lo
    if delta <= 0:
        delta = 1.0

    margin = delta * 0.05
    y_min = math.floor((lo - margin) / 5) * 5
    y_max = math.ceil((hi + margin) / 5) * 5
    if y_max <= y_min:
        y_max = y_min + 5
    return (y_min, y_max)


def plot_dump(
    channels: list[list[float]],
    labels: list[str],
    sample_hz: float,
    title: str,
    out_png: Path,
    *,
    show_saturation: bool = True,
    full_scale: int = FULL_SCALE,
) -> None:
    bits = 12 if full_scale > 255 else 8
    max_len = max(len(ch) for ch in channels)
    t_ms = [i / sample_hz * 1000.0 for i in range(max_len)]
    y_top = full_scale + full_scale // 16

    fig, ax = plt.subplots(figsize=(14, 5))
    for ch_i, (vals, label) in enumerate(zip(channels, labels)):
        offset = ch_i * CHANNEL_PLOT_OFFSET if len(channels) > 1 else 0
        shifted = [v + offset for v in vals]
        suffix = f" +{offset}" if offset else ""
        ax.plot(t_ms[: len(vals)], shifted, lw=0.8, label=f"{label}{suffix} ({bits}-bit)")

    if show_saturation:
        for ch_i, (vals, label) in enumerate(zip(channels, labels)):
            offset = ch_i * CHANNEL_PLOT_OFFSET if len(channels) > 1 else 0
            sat = [i for i, v in enumerate(vals) if v >= full_scale]
            if not sat:
                continue
            ax.scatter(
                [t_ms[i] for i in sat],
                [vals[i] + offset for i in sat],
                s=14,
                zorder=3,
                label=f"{label} sat ({len(sat)})",
            )

    ax.set_title(title)
    ax.set_xlabel(f"time (ms) @ {sample_hz / 1000.0:.1f} kframes/s")
    ax.set_ylabel(f"ADC value ({bits}-bit)")
    ax.set_ylim(-full_scale // 50, y_top + CHANNEL_PLOT_OFFSET * max(0, len(channels) - 1))
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right")

    ax2 = ax.twinx()
    ax2.set_ylim(-full_scale // 50 / full_scale * VREF, y_top / full_scale * VREF)
    ax2.set_ylabel(f"~ voltage (V, Vref={VREF})")

    fig.tight_layout()
    fig.savefig(out_png, dpi=120)
    plt.close(fig)


def plot_phase_snapshot(
    capture: Capture,
    out_png: Path,
    *,
    window: int = 31,
    y_min: float | None = None,
    y_max: float | None = None,
) -> tuple[float, float]:
    """Write a PlotJuggler-like stacked phase snapshot for the latest capture."""
    if not capture.channels or not capture.frames:
        raise ValueError("capture has no samples")

    filtered = lowpass_channels(capture.channels, window)
    auto_min, auto_max = auto_snapshot_ylim(filtered)
    if y_min is None:
        y_min = auto_min
    if y_max is None:
        y_max = auto_max

    frames = capture.frames
    t_ms = [i / capture.sample_hz * 1000.0 for i in range(frames)]

    plot_rows = [
        (phase, ch_idx, adc_label, filtered[ch_idx])
        for phase, ch_idx, adc_label in PHASE_PLOT_ORDER
        if ch_idx < len(filtered)
    ]

    if not plot_rows:
        raise ValueError("capture has no plottable channels")

    out_png.parent.mkdir(parents=True, exist_ok=True)
    fig, axes = plt.subplots(len(plot_rows), 1, figsize=(14, 8), sharex=True)
    if len(plot_rows) == 1:
        axes = [axes]

    colors = {"A": "tab:green", "B": "tab:blue", "C": "tab:red"}
    grid = sector_grid(capture, frames)
    for ax, (phase, _ch_idx, adc_label, vals) in zip(axes, plot_rows):
        color = colors.get(phase, "tab:purple")
        panel_label = f"{phase} ({adc_label})"
        ax.plot(
            t_ms[: len(vals)],
            vals,
            lw=1.4,
            color=color,
            label=f"{panel_label} moving average ({window})",
        )
        if grid:
            fps = grid[1][0] - grid[0][0] if len(grid) > 1 else frames
            for start_frame, idx in grid:
                t0 = start_frame / capture.sample_hz * 1000.0
                ax.axvline(t0, color="gray", lw=0.5, alpha=0.3)
                tc = (start_frame + fps / 2) / capture.sample_hz * 1000.0
                ax.text(
                    tc,
                    0.97,
                    str(idx),
                    transform=ax.get_xaxis_transform(),
                    ha="center",
                    va="top",
                    fontsize=7,
                    color="gray",
                    alpha=0.7,
                )

        if vals:
            v_min = min(vals)
            v_max = max(vals)
            v_mean = sum(vals) / len(vals)
            visible = sum(1 for v in vals if y_min <= v <= y_max)
            ax.set_title(
                f"{panel_label}: filtered min={v_min:.1f} max={v_max:.1f} mean="
                f"{v_mean:.1f} visible={visible}/{len(vals)}"
            )
            if visible == 0:
                ax.text(
                    0.5,
                    0.5,
                    f"trace outside {y_min:g}..{y_max:g}",
                    transform=ax.transAxes,
                    ha="center",
                    va="center",
                    color="tab:red",
                    fontsize=12,
                    alpha=0.8,
                )
        ax.set_ylim(y_min, y_max)
        ax.set_ylabel(phase)
        ax.grid(True, alpha=0.3)
        ax.legend(loc="upper right", fontsize="small")

    axes[-1].set_xlabel(f"time from capture start (ms) @ {capture.sample_hz / 1000.0:.1f} kframes/s")
    fig.suptitle(
        f"latest capture: {frames} frames, moving average window={window}, y={y_min:g}..{y_max:g}"
    )

    fig.tight_layout()
    fig.savefig(out_png, dpi=120)
    plt.close(fig)

    return (y_min, y_max)


def iter_udp_packets(capture: Capture, start_time: float | None = None) -> Iterable[dict[str, float | int]]:
    t0 = time.time() if start_time is None else start_time
    frames = capture.frames
    for idx in range(frames):
        pkt = {
            "plot_ts": t0 + idx / capture.sample_hz,
            "frame_idx": idx,
            "sample_hz": capture.sample_hz,
        }
        for ch, label in enumerate(capture.labels):
            if idx < len(capture.channels[ch]):
                pkt[label] = capture.channels[ch][idx]
        yield pkt


def send_capture_udp(capture: Capture, address: str, port: int, *, realtime: bool = False) -> int:
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    count = 0
    start = time.time()
    try:
        for pkt in iter_udp_packets(capture, start_time=start):
            sock.sendto(json.dumps(pkt).encode("utf-8"), (address, port))
            count += 1
            if realtime:
                target = start + count / capture.sample_hz
                delay = target - time.time()
                if delay > 0:
                    time.sleep(delay)
    finally:
        sock.close()

    return count


def append_event(events: deque[str], message: str, max_events: int = 12) -> None:
    timestamp = time.strftime("%H:%M:%S")
    events.append(f"{timestamp} {message}")
    while len(events) > max_events:
        events.popleft()


# --- six-step zero-crossing analysis -----------------------------------------
#
# Mirrors the physical commutation tables in examples/scope1.rs (each physical
# sector is repeated 4x in the firmware's 24-slot logical table).
SIX_STEP_HIGH = [0, 0, 1, 1, 2, 2]
SIX_STEP_LOW = [1, 2, 2, 0, 0, 1]
PHASE_NAMES = ["A", "B", "C"]
# capture.channels index per phase: A=ch17/PA4, B=ch5/PC4, C=ch14/PB11.
PHASE_TO_CHANNEL = [0, 1, 2]


def frame_is_valley(capture: Capture) -> np.ndarray:
    """Per-frame valley(True)/peak(False) mask for a dual interleaved capture, by
    the per-sector driven-high terminal level. A valley frame drives the high phase
    to ~Vbus; a peak frame collapses it to ~GND. Classify on (high - low) so any
    common offset cancels; the separation is large in either regime down to low duty.
    Returns all-True for a non-interleaved capture (every frame is a valley scan)."""
    n = capture.frames
    if n == 0:
        return np.zeros(0, dtype=bool)
    if not capture.interleaved:
        return np.ones(n, dtype=bool)
    try:
        hz = float(capture.debug.get("hz", "0") or 0)
    except ValueError:
        hz = 0.0
    if hz <= 0:
        return np.ones(n, dtype=bool)
    fps = capture.sample_hz / (hz * 6.0)
    ch = capture.channels
    thr = 0.2 * capture.full_scale
    out = np.empty(n, dtype=bool)
    for i in range(n):
        s = int(i / fps) % 6
        hi, lo = SIX_STEP_HIGH[s], SIX_STEP_LOW[s]
        out[i] = (ch[hi][i] - ch[lo][i]) > thr
    return out


def deinterleave(capture: Capture) -> tuple[Capture, Capture | None]:
    """Split a dual interleaved capture into (valley, peak) sub-captures, each a
    coherent half-rate (20 kHz) stream the normal pipeline (lowpass /
    analyze_zero_crossings / classify_rotor_state) can consume directly. sample_hz
    halves; interleaved=False on both. A non-interleaved capture is returned as
    (capture, None).

    Note: the sub-capture is renumbered from frame 0, so its electrical-zero
    alignment can be off by up to one source frame (≤ half a sub-frame, a few degrees
    at mid band). That is fine for the offset-tolerant consumers above (plateau
    spread, late-swing, qualitative plots). For precise crossing angles, pool the
    float-window samples from the *interleaved* capture using their original frame
    index (frame_is_valley + θ = i/fpr·2π) -- see scope_pv.py."""
    if not capture.interleaved:
        return capture, None
    valley = frame_is_valley(capture)
    vi = [i for i in range(capture.frames) if valley[i]]
    pi = [i for i in range(capture.frames) if not valley[i]]

    def sub(idx: list[int]) -> Capture:
        return Capture(
            channels=[[ch[i] for i in idx] for ch in capture.channels],
            expected=len(idx),
            labels=list(capture.labels),
            sample_hz=capture.sample_hz / 2.0,
            text=capture.text,
            debug=dict(capture.debug),
            regs=list(capture.regs),
            full_scale=capture.full_scale,
            interleaved=False,
        )

    return sub(vi), sub(pi)


@dataclass
class SectorZc:
    """Zero-crossing verdict for one physical sector of the capture."""

    index: int  # sector number from capture start (sector index % 6 = drive state)
    phase: str  # floating phase name
    start_frame: float
    end_frame: float
    status: str  # "zc", "early", or "none"
    zc_frame: float | None = None  # interpolated frame of the crossing
    zc_pct: float | None = None  # crossing position within the sector, 0..100
    direction: str | None = None  # "rise" or "fall"
    d_start: float | None = None  # float-minus-neutral at window start
    d_end: float | None = None  # ... and at window end

    def label(self) -> str:
        if self.status == "zc":
            return f"s{self.index}({self.phase})@{self.zc_pct:.0f}%{'^' if self.direction == 'rise' else 'v'}"
        return f"s{self.index}({self.phase}):{self.status}"


def sector_grid(capture: Capture, frames: int) -> list[tuple[float, int]]:
    """(start_frame, sector 0..5) pairs for a zero-aligned six-step capture.

    Returns [] when the capture has no usable hz or is not six-step.
    """
    try:
        hz = float(capture.debug.get("hz", "0") or 0)
    except ValueError:
        return []
    if hz <= 0 or capture.debug.get("mode") != "six-step":
        return []
    fps = capture.sample_hz / (hz * 6.0)
    out = []
    k = 0
    while k * fps < frames:
        out.append((k * fps, k % 6))
        k += 1
    return out


def _driven_pair_neutral(smooth, frames, fps):
    """Per-sector virtual neutral = the two DRIVEN phase terminals averaged (the
    floating phase EXCLUDED). The float phase carries no current, so the star point
    is set by the driven pair ~= (rail_hi + rail_lo)/2 ~= Vbus/2 at the divider --
    and computed per-frame it tracks bus ripple and divider scale for free.

    Using (A+B+C)/3 instead folds ~1/3 of the floating phase's OWN BEMF into its
    reference (shrinking the measured BEMF to ~2/3) and adds that channel's noise --
    measured ~30% jumpier with ~1.5x smaller BEMF on a locked capture.
    """
    neutral = [0.0] * frames
    k = 0
    while k * fps < frames:
        s = k % 6
        hi_ch = PHASE_TO_CHANNEL[SIX_STEP_HIGH[s]]
        lo_ch = PHASE_TO_CHANNEL[SIX_STEP_LOW[s]]
        i1 = min(int(round((k + 1) * fps)), frames)
        for i in range(int(round(k * fps)), i1):
            neutral[i] = (smooth[hi_ch][i] + smooth[lo_ch][i]) / 2.0
        k += 1
    return neutral


def _neutral_settled_mask(neutral, smooth, frames, tol_frac):
    """True where the driven-pair neutral sits at its steady Vbus/2 level.

    At each sector boundary the just-switched driven terminal rings/demagnetizes
    before settling, so the neutral (its average with the other driven phase)
    departs sharply from the flat ~Vbus/2 baseline -- visible as the spikes/craters
    in the neutral trace. Those frames carry no usable BEMF. Rather than blank a
    fixed N frames (a guess that over-blanks at high RPM and under-blanks at low),
    blank exactly the frames where the neutral is out of band.

    Center = median neutral (the flat baseline dominates). Band = tol_frac of the
    driven rail separation (per-frame max-min of the three terminals), so it scales
    with bus voltage automatically.
    """
    if frames == 0:
        return []
    n0 = statistics.median(neutral[:frames])
    spans = [max(smooth[c][i] for c in range(3)) - min(smooth[c][i] for c in range(3)) for i in range(frames)]
    span = statistics.median(spans) if spans else 0.0
    tol = max(tol_frac * span, 1.0)
    return [abs(neutral[i] - n0) <= tol for i in range(frames)]


def analyze_zero_crossings(
    capture: Capture,
    *,
    smooth_window: int = 5,
    blank_frames: int = 2,
    confirm: int = 2,
    neutral_tol_frac: float = 0.15,
) -> tuple[list[SectorZc], list[list[float]], list[float]]:
    """Locate the float-phase vs virtual-neutral crossing in every sector.

    The capture is zero-aligned (firmware starts the DMA on the electrical-zero
    wrap), so sector k spans frames [k*fps, (k+1)*fps) with
    fps = sample_hz / (hz * 6). The virtual neutral is the mean of all three
    measured terminal voltages, which equals (V_hi + V_lo)/2 exactly at the
    crossing. Returns (sectors, smoothed channels, neutral trace).
    """
    hz = float(capture.debug.get("hz", "0") or 0)
    if hz <= 0:
        raise ValueError("capture debug line has no electrical hz; cannot place sectors")
    if len(capture.channels) < 3:
        raise ValueError("ZC analysis needs all three phase channels")

    smooth = lowpass_channels(capture.channels, smooth_window)
    frames = min(len(ch) for ch in smooth)
    fps = capture.sample_hz / (hz * 6.0)
    neutral = _driven_pair_neutral(smooth, frames, fps)
    settled = _neutral_settled_mask(neutral, smooth, frames, neutral_tol_frac)
    sectors: list[SectorZc] = []
    k = 0
    while (k + 1) * fps <= frames:
        start = k * fps
        end = (k + 1) * fps
        s = k % 6
        hi = SIX_STEP_HIGH[s]
        lo = SIX_STEP_LOW[s]
        fl = 3 - hi - lo
        ch = PHASE_TO_CHANNEL[fl]

        i_hi = int(math.floor(end))
        # Leading blank: at least blank_frames, extended until the neutral has
        # settled out of the boundary transient (data-driven, not a fixed count).
        i0 = int(math.ceil(start)) + blank_frames
        while i0 < i_hi and not settled[i0]:
            i0 += 1
        # Usable frames = the settled ones in [i0, i_hi); interior glitches dropped.
        idxs = [i for i in range(i0, i_hi) if settled[i]]
        zc = SectorZc(
            index=k,
            phase=PHASE_NAMES[fl],
            start_frame=start,
            end_frame=end,
            status="none",
        )
        if len(idxs) >= 3:
            diff = [smooth[ch][i] - neutral[i] for i in idxs]
            zc.d_start = diff[0]
            zc.d_end = diff[-1]
            for j in range(1, len(diff)):
                prev, cur = diff[j - 1], diff[j]
                if prev == 0.0:
                    continue
                falling = prev > 0 and cur <= 0
                rising = prev < 0 and cur >= 0
                if not (falling or rising):
                    continue
                tail = diff[j : j + confirm]
                # Confirmed when the next samples stay on (or at) the new side.
                if len(tail) < confirm or any((v > 0) if falling else (v < 0) for v in tail):
                    continue
                frac = prev / (prev - cur)
                # Interpolate in real frame coords across the (usually adjacent)
                # settled samples idxs[j-1]..idxs[j].
                zc.zc_frame = idxs[j - 1] + frac * (idxs[j] - idxs[j - 1])
                zc.zc_pct = (zc.zc_frame - start) / fps * 100.0
                zc.direction = "rise" if rising else "fall"
                # Crossings on the very first usable pair count as early (still in
                # the settling shadow).
                zc.status = "zc" if j > 1 else "early"
                break
        sectors.append(zc)
        k += 1

    return sectors, smooth, neutral


def classify_rotor_state(
    capture: Capture,
    *,
    smooth_window: int = 5,
    plateau_spread_max: float = 5.0,
    lock_swing: float = 30.0,
) -> dict:
    """Decide locked / stalled / uncertain from a phase-zero-aligned capture WITHOUT
    needing any in-window zero crossing.

    Two stages, both validated on real labeled captures:

    1. SENSING GATE (plateau_spread). The three driven-high plateaus agree to <1%
       when the BEMF divider has time to settle (duty high enough), but diverge to
       25-30% at low duty where the ON pulse is too brief to settle. If the spread
       exceeds plateau_spread_max the float reads (and the (A+B+C)/3 neutral) are
       not trustworthy -> state="uncertain" (low-duty sensing). This is what stops
       the false "locked" calls in the <~6% duty range.

    2. In the trustworthy regime, classify on late_swing: the line-fit excursion of
       (float - neutral) over the LAST ~45% of each window (demag transient
       excluded), median over healthy phases B,C (A skipped for its sense anomaly).
       Rotation keeps ramping to the end of the window; a frozen rotor goes flat
       once demag decays. Spinning measured 82-359; de-energized flat ~1.4.
       late_swing >= lock_swing -> locked, else stalled.

    Thresholds are raw 12-bit counts / percent. The stall cutoff is provisional
    (no clean >6%-duty stalled capture yet); LOCKED and UNCERTAIN are validated.
    bemf_amp (drive-freq sinusoid fit R) is kept as an advisory column only -- it is
    NOT used to classify, because it false-positives on energized stall.
    """
    out = {
        "state": "unknown",
        "sensing": "unknown",
        "plateau_spread": None,
        "late_swing": None,
        "bemf_amp": None,
        "per_phase": {},
        "hz": None,
    }
    try:
        hz = float(capture.debug.get("hz", "0") or 0)
    except ValueError:
        return out
    if hz <= 0 or len(capture.channels) < 3:
        return out
    out["hz"] = hz

    # Stage 1: sensing-quality gate from driven-high plateau agreement.
    highs = [float(np.percentile(np.array(capture.channels[i]), 92)) for i in range(3)]
    mean_high = sum(highs) / 3.0
    spread = (max(highs) - min(highs)) / mean_high * 100.0 if mean_high else 100.0
    out["plateau_spread"] = round(spread, 1)
    out["sensing"] = "ok" if spread <= plateau_spread_max else "poor"

    smooth = lowpass_channels(capture.channels, smooth_window)
    frames = min(len(c) for c in smooth)
    if frames < 6:
        return out
    fps = capture.sample_hz / (hz * 6.0)
    fpr = capture.sample_hz / hz  # frames per electrical rev
    neutral = _driven_pair_neutral(smooth, frames, fps)

    per: dict[str, dict | None] = {}
    for ph in range(3):
        ch = PHASE_TO_CHANNEL[ph]
        thetas, evals, late_swings = [], [], []
        k = 0
        while (k + 1) * fps <= frames:
            s = k % 6
            fl = 3 - SIX_STEP_HIGH[s] - SIX_STEP_LOW[s]
            if fl == ph:
                i0 = int(round(k * fps))
                i1 = int(round((k + 1) * fps))
                idx = list(range(i0, min(i1, frames)))
                if len(idx) >= 3:
                    ev = [smooth[ch][i] - neutral[i] for i in idx]
                    for i, v in zip(idx, ev):
                        thetas.append(2.0 * math.pi * (i / fpr))
                        evals.append(v)
                    a0 = int(len(idx) * 0.55)  # last ~45%, demag excluded
                    if len(idx) - a0 >= 3:
                        xs = np.array(idx[a0:], dtype=float)
                        slope = np.polyfit(xs - xs.mean(), np.array(ev[a0:]), 1)[0]
                        late_swings.append(abs(slope) * (xs[-1] - xs[0]))
            k += 1
        if len(evals) < 4:
            per[PHASE_NAMES[ph]] = None
            continue
        th = np.array(thetas)
        e = np.array(evals)
        design = np.column_stack([np.cos(th), np.sin(th), np.ones_like(th)])
        a, b, _ = np.linalg.lstsq(design, e, rcond=None)[0]
        per[PHASE_NAMES[ph]] = {
            "R": round(float(np.hypot(a, b)), 1),
            "late_swing": round(float(np.mean(late_swings)) if late_swings else 0.0, 1),
        }
    out["per_phase"] = per

    healthy = [per[p] for p in ("B", "C") if per.get(p)]
    if not healthy:
        healthy = [per[p] for p in ("A", "B", "C") if per.get(p)]
    if not healthy:
        return out

    out["bemf_amp"] = round(statistics.median(r["R"] for r in healthy), 1)
    out["late_swing"] = round(statistics.median(r["late_swing"] for r in healthy), 1)

    if out["sensing"] != "ok":
        out["state"] = "uncertain"  # low-duty: float reads / neutral not trustworthy
    elif out["late_swing"] >= lock_swing:
        out["state"] = "locked"
    else:
        out["state"] = "stalled"
    return out


def render_zc_figure(
    capture: Capture,
    fig,
    *,
    smooth_window: int = 5,
    blank_frames: int = 2,
    confirm: int = 2,
    show_current: bool = True,
) -> list[SectorZc]:
    """Draw the 3-phase ZC plot onto an existing figure (cleared first) and return
    the per-sector ZC list. Shared by plot_zc_snapshot (Agg -> PNG) and the live
    streaming viewer (interactive backend -> on-screen window)."""
    sectors, smooth, neutral = analyze_zero_crossings(
        capture,
        smooth_window=smooth_window,
        blank_frames=blank_frames,
        confirm=confirm,
    )

    frames = len(neutral)
    settled = _neutral_settled_mask(neutral, smooth, frames, 0.15)
    t_ms = [i / capture.sample_hz * 1000.0 for i in range(frames)]
    # Scale to the three BEMF voltages only; any extra channels (phase currents,
    # ch16/ch18) ride mid-rail and would skew the voltage plot's y-range.
    y_min, y_max = auto_snapshot_ylim(smooth[:3] + [neutral])

    fig.clf()
    axes = fig.subplots(3, 1, sharex=True)
    colors = {"A": "tab:green", "B": "tab:blue", "C": "tab:red"}
    # Phase -> its current channel (by self-describing header label) for the overlay.
    current_labels = {"A": "ch13", "B": "ch16", "C": "ch18"}
    cur_color = "#7d5ba6"  # muted purple: distinct from the RGB voltages, gray, orange

    for phase_idx, ax in enumerate(axes):
        phase = PHASE_NAMES[phase_idx]
        ch = PHASE_TO_CHANNEL[phase_idx]
        color = colors[phase]

        # Gentle current overlay: the matching phase current on a behind-the-voltage
        # twinx, pushed into the lower third (generous top padding) so it never
        # competes with the neutral/ZC story mid-plot. Only for captures carrying the
        # current channels (7-ch dumps); 3-ch dumps skip it untouched.
        if show_current:
            cur_lbl = current_labels.get(phase)
            ci = capture.labels.index(cur_lbl) if cur_lbl in capture.labels else None
            if ci is not None and ci < len(smooth):
                cur = smooth[ci][:frames]
                cmin, cmax = min(cur), max(cur)
                span = max(cmax - cmin, 1.0)
                axc = ax.twinx()
                axc.plot(t_ms, cur, lw=0.9, color=cur_color, alpha=0.55, zorder=1)
                axc.set_ylim(cmin - 0.3 * span, cmax + 2.4 * span)
                axc.set_zorder(ax.get_zorder() - 1)  # behind the voltage axis
                ax.patch.set_visible(False)  # let the current show through
                axc.tick_params(axis="y", labelsize=6, colors=cur_color)
                axc.set_ylabel(f"I_{phase}", fontsize=7, color=cur_color)

        ax.plot(t_ms, smooth[ch][:frames], lw=1.2, color=color, label=f"{phase} (smoothed {smooth_window})")
        ax.plot(t_ms, neutral, lw=0.9, ls="--", color="gray", label="virtual neutral (driven pair)")
        # Mark frames the neutral flags as unsettled (boundary transient) -> blanked.
        i = 0
        while i < frames:
            if not settled[i]:
                j = i
                while j < frames and not settled[j]:
                    j += 1
                ax.axvspan(t_ms[i], t_ms[min(j, frames - 1)], color="0.5", alpha=0.18, lw=0)
                i = j
            else:
                i += 1

        zc_count = 0
        for sec in sectors:
            t0 = sec.start_frame / capture.sample_hz * 1000.0
            t1 = sec.end_frame / capture.sample_hz * 1000.0
            ax.axvline(t0, color="black", lw=0.5, alpha=0.25)
            is_float = sec.phase == phase
            if is_float:
                ax.axvspan(t0, t1, color=color, alpha=0.07)
            ax.text(
                (t0 + t1) / 2,
                0.97,
                str(sec.index % 6),
                transform=ax.get_xaxis_transform(),
                ha="center",
                va="top",
                fontsize=8 if is_float else 7,
                fontweight="bold" if is_float else "normal",
                color=color if is_float else "gray",
                alpha=0.9 if is_float else 0.6,
            )
            if not is_float:
                continue
            if sec.zc_frame is not None:
                zc_count += 1
                tz = sec.zc_frame / capture.sample_hz * 1000.0
                vz = neutral[min(int(round(sec.zc_frame)), frames - 1)]
                marker_color = "black" if sec.status == "zc" else "tab:orange"
                ax.axvline(tz, color=marker_color, lw=1.2, ls=":")
                ax.plot([tz], [vz], "o", ms=5, color=marker_color, zorder=4)
                ax.annotate(
                    f"{sec.zc_pct:.0f}%",
                    (tz, vz),
                    textcoords="offset points",
                    xytext=(3, 8),
                    fontsize=7,
                    color=marker_color,
                )

        found = sum(1 for s in sectors if s.phase == phase and s.status == "zc")
        total = sum(1 for s in sectors if s.phase == phase)
        ax.set_title(f"{phase}: in-window ZC {found}/{total} sectors")
        ax.set_ylim(y_min, y_max)
        ax.set_ylabel(f"{phase} (12-bit)" if capture.full_scale > 255 else phase)
        ax.grid(True, alpha=0.25)
        ax.legend(loc="upper right", fontsize="small")

    axes[-1].set_xlabel(f"time from electrical zero (ms) @ {capture.sample_hz / 1000.0:.1f} kframes/s")
    hz = capture.debug.get("hz", "?")
    amp = capture.debug.get("amp", "?")
    mode = capture.debug.get("mode", "?")
    fps = capture.sample_hz / (float(hz) * 6.0) if hz not in ("?", "0") else 0.0
    in_window = sum(1 for s in sectors if s.status == "zc")
    rotor = classify_rotor_state(capture, smooth_window=smooth_window)
    state = rotor["state"].upper()
    state_color = {
        "LOCKED": "tab:green",
        "STALLED": "tab:red",
        "UNCERTAIN": "tab:gray",
    }.get(state, "black")
    fig.suptitle(
        f"rotor={state}  (late_swing={rotor['late_swing']} plateau_spread={rotor['plateau_spread']}% "
        f"sensing={rotor['sensing']})   |   "
        f"mode={mode} hz={hz} amp={amp} | {frames} frames, {fps:.1f} frames/sector | "
        f"in-window ZC {in_window}/{len(sectors)} sectors",
        color=state_color,
        fontweight="bold",
    )

    fig.tight_layout()
    return sectors


def plot_zc_snapshot(
    capture: Capture,
    out_png: Path,
    *,
    smooth_window: int = 5,
    blank_frames: int = 2,
    confirm: int = 2,
    show_current: bool = True,
) -> list[SectorZc]:
    """Render the three phases with virtual neutral, sector grid and exact ZC marks
    into a PNG."""
    out_png.parent.mkdir(parents=True, exist_ok=True)
    fig = plt.figure(figsize=(14, 9))
    sectors = render_zc_figure(
        capture,
        fig,
        smooth_window=smooth_window,
        blank_frames=blank_frames,
        confirm=confirm,
        show_current=show_current,
    )
    fig.savefig(out_png, dpi=120)
    plt.close(fig)
    return sectors


def render_envelope_figure(capture: Capture, fig, *, blank_frames: int = 2):
    """Dual peak+valley per-channel envelope (the scope2-only diagram). For each
    BEMF channel, overlay the valley samples (ON sub-period, ~Vbus rail, full
    bipolar BEMF) and the peak samples (OFF sub-period, ~GND rail, clamped BEMF),
    plus the driven-pair virtual neutral evaluated on each population -- the valley
    neutral (~Vbus/2) is the ZC datum the valley trace crosses; the peak neutral
    (~GND) is the floor the clamped trace lifts off. Green shading = sectors where
    that phase floats; the first `blank_frames` of each sector (commutation/demag)
    are greyed. Returns the axes. Intended for interleaved captures."""
    axes = fig.subplots(3, 1, sharex=True)
    n = capture.frames
    try:
        hz = float(capture.debug.get("hz", "0") or 0)
    except ValueError:
        hz = 0.0
    if n == 0 or hz <= 0:
        return axes
    fps = capture.sample_hz / (hz * 6.0)
    fpr = capture.sample_hz / hz
    ch = capture.channels
    valley = frame_is_valley(capture)
    idx = np.arange(n)
    ang = idx / fpr * 360.0
    sec = (idx / fps).astype(int) % 6
    neut = np.array(
        [(ch[SIX_STEP_HIGH[sec[i]]][i] + ch[SIX_STEP_LOW[sec[i]]][i]) / 2.0 for i in range(n)]
    )
    demag = (idx - (idx / fps).astype(int) * fps) < blank_frames
    full = capture.full_scale
    nsec = int(np.ceil(ang[-1] / 60.0)) + 1 if n else 0
    for cidx, ax in enumerate(axes):
        chan = np.array(ch[cidx])
        v = valley & ~demag
        p = ~valley & ~demag
        ax.plot(ang[v], chan[v], ".-", ms=4, lw=0.8, color="tab:red", label="valley (ON / ~Vbus)")
        ax.plot(ang[p], chan[p], ".-", ms=4, lw=0.8, color="tab:blue", label="peak (OFF / ~GND)")
        ax.plot(ang[v], neut[v], "--", lw=1.3, color="darkred", alpha=0.9, label="valley neutral")
        ax.plot(ang[p], neut[p], "--", lw=1.0, color="navy", alpha=0.7, label="peak neutral")
        if demag.any():
            ax.plot(ang[demag], chan[demag], ".", ms=3, color="0.6", alpha=0.5)
        ax.set_ylabel(f"{PHASE_NAMES[cidx]} / {capture.labels[cidx]}", fontsize=9)
        ax.grid(alpha=0.3)
        ax.set_ylim(-0.03 * full, 1.05 * full)
        if cidx == 0:
            ax.legend(fontsize=7, loc="center right", ncol=2)
        for k in range(nsec):
            s = k % 6
            if 3 - SIX_STEP_HIGH[s] - SIX_STEP_LOW[s] == cidx:
                ax.axvspan(k * 60, (k + 1) * 60, color="green", alpha=0.06)
    amp = capture.debug.get("amp", "?")
    axes[-1].set_xlabel(
        "electrical angle (deg) — green = phase floating; ZC = valley trace ∩ valley neutral"
    )
    fig.suptitle(
        f"Dual peak+valley envelope — {int(hz)} Hz amp={amp} "
        f"({capture.sample_hz:.0f} Hz, {n} frames)\n"
        "red=valley  blue=peak  dark-red dashed=valley neutral (Vbus/2)  navy dashed=peak neutral"
    )
    fig.tight_layout()
    return axes


def plot_envelope_snapshot(capture: Capture, out_png: Path, *, blank_frames: int = 2):
    """Render the dual envelope figure to a PNG."""
    out_png.parent.mkdir(parents=True, exist_ok=True)
    fig = plt.figure(figsize=(11, 8.5))
    render_envelope_figure(capture, fig, blank_frames=blank_frames)
    fig.savefig(out_png, dpi=120)
    plt.close(fig)


def format_zc_report(sectors: list[SectorZc]) -> str:
    """Multi-line per-sector table for the latest ZC analysis."""
    lines = ["sector phase status   zc_frame   zc_pct  dir   d_start   d_end"]
    for s in sectors:
        if s.zc_frame is not None:
            zc_frame = f"{s.zc_frame:8.2f}"
            zc_pct = f"{s.zc_pct:6.1f}%"
            direction = s.direction or "-"
        else:
            zc_frame = "       -"
            zc_pct = "      -"
            direction = "-"
        d_start = f"{s.d_start:8.1f}" if s.d_start is not None else "       -"
        d_end = f"{s.d_end:8.1f}" if s.d_end is not None else "       -"
        lines.append(
            f"s{s.index:<5d} {s.phase}     {s.status:<8s} {zc_frame} {zc_pct} {direction:<5s} {d_start} {d_end}"
        )
    return "\n".join(lines)
