#!/usr/bin/env python3
"""Shared helpers for rinz scope UART captures and PlotJuggler replay."""

from __future__ import annotations

from dataclasses import dataclass, field
import json
import math
import re
import socket
import time
from collections import deque
from pathlib import Path
from typing import Iterable

import matplotlib

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
            if line.lstrip().startswith("dump:")
            or line.lstrip().startswith("dump3:")
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
    is_dump3 = header.startswith("dump3:") or is_cap
    m = re.search(r"dump3?:\s*(\d+)", header)
    if m is None and is_cap:
        m = re.search(r"\bframes=(\d+)", header)
    expected = int(m.group(1)) if m else None
    sample_hz_match = re.search(r"(\d+(?:\.\d+)?)\s*Hz", header)
    if sample_hz_match is None and is_cap:
        sample_hz_match = re.search(r"\bsample_hz=(\d+(?:\.\d+)?)", header)
    header_sample_hz = float(sample_hz_match.group(1)) if sample_hz_match else None

    body = []
    for line in lines[start + 1 :]:
        body.append(line)
        if "end" in line or "CAP_END" in line:
            break

    is_12bit = "12-bit" in header
    hex_pat = "[0-9a-fA-F]{4}" if is_12bit else "[0-9a-fA-F]{2}"
    full_scale = 4095 if is_12bit else FULL_SCALE

    blob = " ".join(body).replace("CAP_DATA", " ").replace("CAP_END", " ").replace("end", " ")
    vals = [int(tok, 16) for tok in blob.split() if re.fullmatch(hex_pat, tok)]

    if not is_dump3:
        channels = [vals]
        labels = ["ch17"]
        sample_hz = header_sample_hz or SAMPLE_HZ
    else:
        usable = len(vals) - len(vals) % 3
        frames = vals[:usable]
        channels = [frames[i::3] for i in range(3)]
        labels = DEFAULT_LABELS.copy()
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
    )


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


def analyze_zero_crossings(
    capture: Capture,
    *,
    smooth_window: int = 5,
    blank_frames: int = 2,
    confirm: int = 2,
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
    neutral = [(smooth[0][i] + smooth[1][i] + smooth[2][i]) / 3.0 for i in range(frames)]

    fps = capture.sample_hz / (hz * 6.0)
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

        i0 = int(math.ceil(start)) + blank_frames
        i1 = int(math.floor(end))
        zc = SectorZc(
            index=k,
            phase=PHASE_NAMES[fl],
            start_frame=start,
            end_frame=end,
            status="none",
        )
        if i1 - i0 >= 2:
            diff = [smooth[ch][i] - neutral[i] for i in range(i0, i1)]
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
                zc.zc_frame = (i0 + j - 1) + frac
                zc.zc_pct = (zc.zc_frame - start) / fps * 100.0
                zc.direction = "rise" if rising else "fall"
                # Crossings inside the blanking shadow count as early.
                zc.status = "zc" if j > 1 else "early"
                break
        sectors.append(zc)
        k += 1

    return sectors, smooth, neutral


def plot_zc_snapshot(
    capture: Capture,
    out_png: Path,
    *,
    smooth_window: int = 5,
    blank_frames: int = 2,
    confirm: int = 2,
) -> list[SectorZc]:
    """Render the three phases with virtual neutral, sector grid and exact ZC marks."""
    sectors, smooth, neutral = analyze_zero_crossings(
        capture,
        smooth_window=smooth_window,
        blank_frames=blank_frames,
        confirm=confirm,
    )

    frames = len(neutral)
    t_ms = [i / capture.sample_hz * 1000.0 for i in range(frames)]
    y_min, y_max = auto_snapshot_ylim(smooth + [neutral])

    out_png.parent.mkdir(parents=True, exist_ok=True)
    fig, axes = plt.subplots(3, 1, figsize=(14, 9), sharex=True)
    colors = {"A": "tab:green", "B": "tab:blue", "C": "tab:red"}

    for phase_idx, ax in enumerate(axes):
        phase = PHASE_NAMES[phase_idx]
        ch = PHASE_TO_CHANNEL[phase_idx]
        color = colors[phase]
        ax.plot(t_ms, smooth[ch][:frames], lw=1.2, color=color, label=f"{phase} (smoothed {smooth_window})")
        ax.plot(t_ms, neutral, lw=0.9, ls="--", color="gray", label="virtual neutral (A+B+C)/3")

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
    fig.suptitle(
        f"zero crossings: mode={mode} hz={hz} amp={amp} | {frames} frames, "
        f"{fps:.1f} frames/sector | in-window ZC {in_window}/{len(sectors)} sectors"
    )

    fig.tight_layout()
    fig.savefig(out_png, dpi=120)
    plt.close(fig)

    return sectors


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
