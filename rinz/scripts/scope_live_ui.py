#!/usr/bin/env python3
"""Blessed UART UI for rinz motor scope captures and PlotJuggler replay."""

from __future__ import annotations

import argparse
from collections import deque
from dataclasses import dataclass, field
from datetime import datetime
import json
from pathlib import Path
import re
import shutil
import sys
import threading
import time

try:
    from blessed import Terminal
except ImportError as exc:
    raise SystemExit("missing dependency: pip install blessed") from exc

try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

from scope_common import (
    AMP_START_TENTHS,
    BAUD,
    Capture,
    FREQ_START_HZ,
    FREQ_STEP_HZ,
    analyze_zero_crossings,
    append_event,
    classify_rotor_state,
    format_zc_report,
    parse_capture,
    plot_phase_snapshot,
    plot_zc_snapshot,
    read_available,
    send_capture_udp,
    split_complete_dumps,
)


term = Terminal()
CAPTURE_TIMEOUT_S = 12.0
SNAPSHOT_WINDOW = 31
# None = auto-scale from the filtered traces (plot_phase_snapshot default).
SNAPSHOT_YMIN: float | None = None
SNAPSHOT_YMAX: float | None = None


@dataclass
class UiState:
    port: str
    baud: int
    udp_address: str
    udp_port: int
    mode: str = "six-step"
    hz: int = 60
    amp: float = 8.0
    trim: int = 0  # raw CCR duty trim (firmware '[' / ']'), reported via 'trim=' echo
    connected: bool = False
    running: bool = True
    capturing: bool = False
    postprocessing: bool = False
    capture_started_at: float | None = None
    capture_text: str = ""
    last_capture: Capture | None = None
    bytes_rx: int = 0
    last_bytes_rx: int = 0
    bytes_per_sec: float = 0.0
    udp_packets: int = 0
    udp_packets_last: int = 0
    udp_packets_per_sec: float = 0.0
    parse_errors: int = 0
    last_line: str = ""
    status: str = "Starting"
    events: deque[str] = field(default_factory=lambda: deque(maxlen=14))
    debug: dict[str, str] = field(default_factory=dict)
    regs: list[str] = field(default_factory=list)
    loop_replay: bool = False
    last_replay_at: float = 0.0
    udp_replaying: bool = False
    comm_log_path: Path | None = None
    snapshot_path: Path = Path("logs/latest_snapshot.png")
    snapshot_log_path: Path = Path("logs/latest_snapshot.log")
    snapshot_window: int = SNAPSHOT_WINDOW
    snapshot_ymin: float | None = SNAPSHOT_YMIN
    snapshot_ymax: float | None = SNAPSHOT_YMAX
    zc_path: Path = Path("logs/latest_zc.png")
    zc_log_path: Path = Path("logs/latest_zc.txt")
    zc_window: int = 3
    zc_summary: str = "no ZC analysis yet"
    archive_dir: Path = Path("logs/captures")
    # Live streaming ('l' start / 'k' stop): firmware dumps back-to-back and we
    # regenerate zc_path continuously for a passive viewer (scope_view.py).
    streaming: bool = False
    stream_buffer: str = ""
    stream_dumps: int = 0
    stream_last_render: float = 0.0
    stream_rendering: bool = False
    stream_refresh: float = 1.0
    exploration_path: Path = Path("logs/exploration.json")
    rotor_summary: str = "rotor=?"
    # Every streamed dump is appended here (a fresh timestamped file per 'l') so a
    # ramp-through-dropout sequence can be analyzed offline.
    stream_archive_path: Path | None = None
    stream_archive_idx: int = 0


class CommLog:
    def __init__(self, path: Path | None) -> None:
        self.path = path
        self.file = None
        if path is not None:
            path.parent.mkdir(parents=True, exist_ok=True)
            self.file = path.open("a", encoding="utf-8", buffering=1)
            self.event(f"opened port log path={path}")

    def close(self) -> None:
        if self.file is not None:
            self.event("closing port log")
            self.file.close()
            self.file = None

    def event(self, message: str) -> None:
        self.write("EVT", message)

    def write(self, direction: str, payload: str) -> None:
        if self.file is None:
            return
        timestamp = datetime.now().isoformat(timespec="milliseconds")
        escaped = payload.encode("unicode_escape", errors="replace").decode("ascii")
        self.file.write(f"{timestamp} {direction} {escaped}\n")


def normalize_mode(mode: str) -> str:
    return "sine" if mode == "sine" else "six-step"


def target_hz_arg(hz: float) -> int:
    return int(round(hz / 10.0) * 10)


def format_stats(capture: Capture | None) -> str:
    if capture is None:
        return "no capture"
    stats = capture.stats
    return (
        f"{capture.frames} frames @ {capture.sample_hz / 1000:.1f} kHz | "
        f"min={stats['min']} max={stats['max']} mean={stats['mean']:.1f} sat={stats['sat']}"
    )


def parse_status_line(state: UiState, line: str) -> None:
    stripped = line.strip()
    if not stripped:
        return
    state.last_line = stripped

    if stripped.startswith("mode="):
        state.mode = normalize_mode(stripped.split("=", 1)[1])
    elif stripped.startswith("trim="):
        try:
            state.trim = int(stripped.split("=", 1)[1].split()[0])
        except (ValueError, IndexError):
            pass
    elif stripped.startswith("freq=") and stripped.endswith("Hz"):
        try:
            state.hz = int(stripped.split("=", 1)[1].removesuffix("Hz"))
        except ValueError:
            pass
    elif stripped.startswith("amp=") and stripped.endswith("%"):
        try:
            state.amp = float(stripped.split("=", 1)[1].removesuffix("%"))
        except ValueError:
            pass
    elif stripped.startswith("reset:"):
        state.mode = "six-step"
        # Example: reset: freq=60Hz amp=8.0% mode=six-step
        for item in stripped.split():
            if item.startswith("freq=") and item.endswith("Hz"):
                try:
                    state.hz = int(item.split("=", 1)[1].removesuffix("Hz"))
                except ValueError:
                    pass
            elif item.startswith("amp=") and item.endswith("%"):
                try:
                    state.amp = float(item.split("=", 1)[1].removesuffix("%"))
                except ValueError:
                    pass
        append_event(state.events, stripped)
    elif stripped.startswith("debug:"):
        state.debug = {}
        for item in stripped.split()[1:]:
            if "=" in item:
                key, value = item.split("=", 1)
                state.debug[key] = value
        append_event(state.events, stripped)
    elif stripped.startswith("DBG "):
        state.debug = {}
        for item in stripped.split()[1:]:
            if "=" in item:
                key, value = item.split("=", 1)
                state.debug[key] = value
        append_event(state.events, stripped)
    elif stripped.startswith("EVT "):
        append_event(state.events, stripped)
    elif stripped.startswith("regs:"):
        state.regs.append(stripped)
        state.regs = state.regs[-4:]
    elif stripped in {"kill", "notch=off", "notch=A", "notch=B", "notch=C"}:
        append_event(state.events, stripped)


def read_available_logged(ser: serial.Serial, log: CommLog, idle_s: float = 0.15, max_s: float = 2.0) -> str:
    text = read_available(ser, idle_s=idle_s, max_s=max_s)
    if text:
        log.write("RX", text)
    return text


def send_raw_key(
    ser: serial.Serial,
    state: UiState,
    log: CommLog,
    key: str,
    description: str | None = None,
) -> None:
    log.write("TX", key)
    ser.write(key.encode("ascii"))
    ser.flush()
    if description:
        append_event(state.events, description)
        log.event(description)


def send_key_logged(
    ser: serial.Serial,
    state: UiState,
    log: CommLog,
    key: str,
    delay_s: float = 0.04,
) -> str:
    send_raw_key(ser, state, log, key)
    time.sleep(delay_s)
    return read_available_logged(ser, log, idle_s=0.04, max_s=0.4)


def send_keys_logged(ser: serial.Serial, state: UiState, log: CommLog, key: str, count: int) -> str:
    out = []
    for _ in range(count):
        out.append(send_key_logged(ser, state, log, key))
    return "".join(out)


def ramp_amplitude_logged(ser: serial.Serial, state: UiState, log: CommLog, target_percent: float) -> str:
    # Ramp from the firmware's ACTUAL current amp (parsed from its reset/amp echoes into
    # state.amp), not a hardcoded assumption -- the firmware's reset amp (AMP_START) is
    # 11.0%, not AMP_START_TENTHS (8.0%), which silently added +3% to every --amp target.
    target = int(round(target_percent * 10))
    delta = target - int(round(state.amp * 10))
    out = []

    full_steps, tenths = divmod(abs(delta), 10)
    if delta >= 0:
        out.append(send_keys_logged(ser, state, log, "a", full_steps))
        out.append(send_keys_logged(ser, state, log, "+", tenths))
    else:
        out.append(send_keys_logged(ser, state, log, "z", full_steps))
        out.append(send_keys_logged(ser, state, log, "-", tenths))

    return "".join(out)


def ramp_frequency_logged(ser: serial.Serial, state: UiState, log: CommLog, target_hz: int) -> str:
    delta = target_hz - int(round(state.hz)) # ramp from the actual parsed hz, not a constant
    steps = abs(delta) // FREQ_STEP_HZ
    return send_keys_logged(ser, state, log, "f" if delta >= 0 else "v", steps)


def start_capture(ser: serial.Serial, state: UiState, log: CommLog, key: str = "d") -> None:
    state.capturing = True
    state.capture_text = ""
    state.capture_started_at = time.time()
    fmt = "binary" if key == "c" else "hex"
    state.status = f"Capture requested ({fmt}); waiting for dump bytes"
    send_raw_key(ser, state, log, key, f"capture requested ({fmt})")


def _archive_capture(state: UiState, log: CommLog, capture: Capture, with_zc: bool) -> str:
    """Copy the freshly written latest_* outputs to a numbered capNNN_* set."""
    base = state.archive_dir
    base.mkdir(parents=True, exist_ok=True)
    nums = []
    for p in base.glob("cap*"):
        m = re.match(r"cap(\d+)_", p.name)
        if m:
            nums.append(int(m.group(1)))
    n = max(nums, default=0) + 1
    mode = capture.debug.get("mode", "unk")
    hz = capture.debug.get("hz", "x")
    amp = capture.debug.get("amp", "x")
    prefix = f"cap{n:03d}_{mode}_hz{hz}_amp{amp}"

    pairs = [
        (state.snapshot_path, "_snapshot.png"),
        (state.snapshot_log_path, "_raw.log"),
    ]
    if with_zc:
        pairs += [(state.zc_path, "_zc.png"), (state.zc_log_path, "_zc.txt")]
    for src, suffix in pairs:
        if src.exists():
            shutil.copyfile(src, base / (prefix + suffix))
    log.event(f"archived {base / prefix}_*")
    return prefix


def _finish_capture_worker(state: UiState, log: CommLog, capture_text: str) -> None:
    try:
        capture = parse_capture(capture_text)
    except Exception as exc:
        state.parse_errors += 1
        state.status = f"Capture parse failed: {exc}"
        append_event(state.events, state.status)
        log.event(state.status)
        return

    state.last_capture = capture
    state.debug = capture.debug or state.debug
    state.regs = capture.regs or state.regs
    try:
        state.snapshot_log_path.parent.mkdir(parents=True, exist_ok=True)
        state.snapshot_log_path.write_text(capture_text, encoding="ascii", errors="replace")
        log.event(f"snapshot raw log {state.snapshot_log_path}")
    except Exception as exc:
        log.event(f"snapshot raw log failed: {exc}")
    try:
        plot_phase_snapshot(
            capture,
            state.snapshot_path,
            window=state.snapshot_window,
            y_min=state.snapshot_ymin,
            y_max=state.snapshot_ymax,
        )
        snapshot_msg = f"; snapshot {state.snapshot_path}"
    except Exception as exc:
        snapshot_msg = f"; snapshot failed: {exc}"
        state.parse_errors += 1
        log.event(f"snapshot failed: {exc}")

    with_zc = False
    # Firmware is six-step only now (sine removed), so its debug line no longer
    # carries mode=. Gate the ZC overlay on hz alone; default mode for the title.
    if capture.debug.get("hz"):
        capture.debug.setdefault("mode", "six-step")
        try:
            sectors = plot_zc_snapshot(capture, state.zc_path, smooth_window=state.zc_window)
            state.zc_log_path.parent.mkdir(parents=True, exist_ok=True)
            state.zc_log_path.write_text(format_zc_report(sectors), encoding="ascii")
            with_zc = True
            in_window = sum(1 for s in sectors if s.status == "zc")
            first_rev = " ".join(s.label() for s in sectors[:6])
            state.zc_summary = f"ZC {in_window}/{len(sectors)} in-window | {first_rev}"
            rotor = classify_rotor_state(capture, smooth_window=state.zc_window)
            state.rotor_summary = (
                f"rotor={rotor['state'].upper()} late_swing={rotor['late_swing']} "
                f"spread={rotor['plateau_spread']}% sensing={rotor['sensing']}"
            )
            snapshot_msg += f"; zc {state.zc_path}"
            append_event(state.events, state.zc_summary)
            log.event(state.zc_summary)
        except Exception as exc:
            state.zc_summary = f"ZC analysis failed: {exc}"
            snapshot_msg += f"; zc failed: {exc}"
            log.event(state.zc_summary)

    try:
        prefix = _archive_capture(state, log, capture, with_zc)
        snapshot_msg += f"; archived {prefix}"
    except Exception as exc:
        log.event(f"archive failed: {exc}")

    state.status = f"Capture parsed: {format_stats(capture)}{snapshot_msg}"
    append_event(state.events, state.status)
    log.event(state.status)


def finish_capture(state: UiState, log: CommLog) -> None:
    capture_text = state.capture_text
    nbytes = len(capture_text)
    state.status = f"Capture complete ({nbytes} bytes); processing snapshot..."
    append_event(state.events, state.status)
    log.event(state.status)
    state.postprocessing = True

    def worker() -> None:
        try:
            _finish_capture_worker(state, log, capture_text)
        finally:
            state.postprocessing = False

    threading.Thread(target=worker, daemon=True, name="capture-postprocess").start()


def _render_stream(state: UiState, log: CommLog, dump_text: str) -> None:
    """Regenerate zc_path from one streamed dump, off the UI thread. Writes via a
    temp file + atomic replace so a passive viewer never reads a half-written PNG."""
    state.stream_rendering = True

    def worker() -> None:
        try:
            capture = parse_capture(dump_text)
            capture.debug.setdefault("mode", "six-step")
            state.last_capture = capture
            if capture.debug:
                state.debug = capture.debug
            tmp = state.zc_path.with_suffix(".tmp.png")
            sectors = plot_zc_snapshot(capture, tmp, smooth_window=state.zc_window)
            tmp.replace(state.zc_path)
            in_window = sum(1 for s in sectors if s.status == "zc")
            state.zc_summary = f"stream ZC {in_window}/{len(sectors)} in-window (dump {state.stream_dumps})"
            rotor = classify_rotor_state(capture, smooth_window=state.zc_window)
            state.rotor_summary = (
                f"rotor={rotor['state'].upper()} late_swing={rotor['late_swing']} "
                f"spread={rotor['plateau_spread']}% sensing={rotor['sensing']}"
            )
        except Exception as exc:
            log.event(f"stream render failed: {exc}")
        finally:
            state.stream_rendering = False

    threading.Thread(target=worker, daemon=True, name="stream-render").start()


def _int_or_none(value):
    try:
        return int(value)
    except (TypeError, ValueError):
        return value


def _arr_from_regs(regs: list[str]):
    """Pull TIM1 ARR (hex) out of the firmware 'regs:' lines, or None."""
    for line in regs:
        m = re.search(r"t1_arr=([0-9a-fA-F]+)", line)
        if m:
            return int(m.group(1), 16)
    return None


def log_exploration_point(state: UiState, log: CommLog) -> None:
    """Append the current operating point + ZC analysis to exploration.json."""
    capture = state.last_capture
    if capture is None:
        state.status = "No capture yet to log (run 'd' or start streaming with 'l')"
        append_event(state.events, state.status)
        return
    try:
        sectors, _smooth, _neutral = analyze_zero_crossings(capture, smooth_window=state.zc_window)
    except Exception as exc:
        state.status = f"Exploration analysis failed: {exc}"
        append_event(state.events, state.status)
        log.event(state.status)
        return

    amp_tenths = _int_or_none(capture.debug.get("amp"))
    trim = _int_or_none(capture.debug.get("trim", state.trim))
    # Effective six-step duty in raw CCR counts: matches the firmware exactly --
    # base = arr*amp*2/(1000*3) (integer div), then + trim, clamped to [0, arr].
    arr = _arr_from_regs(capture.regs)
    duty_ct = None
    duty_pct = None
    if isinstance(amp_tenths, int) and arr is not None:
        base = arr * amp_tenths * 2 // (1000 * 3)
        duty_ct = max(0, min(base + (trim if isinstance(trim, int) else 0), arr))
        duty_pct = round(duty_ct / arr * 100.0, 2)
    record = {
        "ts": datetime.now().isoformat(timespec="seconds"),
        "hz": _int_or_none(capture.debug.get("hz")),
        "amp_tenths": amp_tenths,
        "amp_pct": (amp_tenths / 10.0) if isinstance(amp_tenths, int) else None,
        "trim": trim,
        "arr": arr,
        "duty_ct": duty_ct,
        "duty_pct": duty_pct,
        "frames": capture.frames,
        "sample_hz": capture.sample_hz,
        "zc_window": state.zc_window,
        "mode": capture.debug.get("mode", "six-step"),
        "in_window_zc": sum(1 for s in sectors if s.status == "zc"),
        "total_sectors": len(sectors),
        "rotor": classify_rotor_state(capture, smooth_window=state.zc_window),
        "sectors": [
            {
                "index": s.index,
                "sector_type": s.index % 6,
                "phase": s.phase,
                "status": s.status,
                "zc_pct": round(s.zc_pct, 2) if s.zc_pct is not None else None,
                "zc_frame": round(s.zc_frame, 3) if s.zc_frame is not None else None,
                "direction": s.direction,
                "d_start": round(s.d_start, 1) if s.d_start is not None else None,
                "d_end": round(s.d_end, 1) if s.d_end is not None else None,
            }
            for s in sectors
        ],
        "debug": capture.debug,
        "regs": capture.regs,
    }

    path = state.exploration_path
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        records = []
        if path.exists():
            try:
                loaded = json.loads(path.read_text(encoding="utf-8"))
                records = loaded if isinstance(loaded, list) else [loaded]
            except Exception:
                records = []
        record["n"] = len(records) + 1
        records.append(record)
        path.write_text(json.dumps(records, indent=2), encoding="utf-8")
        state.status = (
            f"Logged point #{record['n']} -> {path.name}: hz={record['hz']} "
            f"amp={record['amp_pct']}% trim={record['trim']} duty={record['duty_ct']}ct "
            f"ZC {record['in_window_zc']}/{record['total_sectors']}"
        )
        append_event(state.events, state.status)
        log.event(state.status)
    except Exception as exc:
        state.status = f"Exploration write failed: {exc}"
        append_event(state.events, state.status)
        log.event(state.status)


def poll_serial(ser: serial.Serial, state: UiState, log: CommLog) -> None:
    n = ser.in_waiting
    if not n:
        return
    data = ser.read(n).decode("ascii", errors="replace")
    state.bytes_rx += len(data)

    if state.capturing:
        state.capture_text += data
        # Do not write the full hex dump to the comm log; it blocks the UI loop.
        log.write(
            "RX",
            f"<capture chunk {len(data)} bytes, total {len(state.capture_text)} bytes>",
        )
        # Line-anchored terminator: b85 (cdump) payload can contain "end" mid-stream.
        if re.search(r"[\r\n]end\b", state.capture_text) or "CAP_END" in state.capture_text:
            state.capturing = False
            finish_capture(state, log)
        return

    if state.streaming:
        state.stream_buffer += data
        segments, state.stream_buffer = split_complete_dumps(state.stream_buffer)
        if segments:
            state.stream_dumps += len(segments)
            # Persist every dump (self-describing: each carries hz/amp/trim in its
            # debug line) so a ramp-through-dropout sequence is recoverable offline.
            if state.stream_archive_path is not None:
                try:
                    with state.stream_archive_path.open("a", encoding="ascii", errors="replace") as fh:
                        for seg in segments:
                            ts = datetime.now().isoformat(timespec="milliseconds")
                            fh.write(f"# dump n={state.stream_archive_idx} t={ts}\n{seg}end\n")
                            state.stream_archive_idx += 1
                except Exception as exc:
                    log.event(f"stream archive failed: {exc}")
            now = time.time()
            # Keep only the latest dump; redraw at most once per stream_refresh and
            # never while a previous render is still in flight (keeps the UI snappy).
            if not state.stream_rendering and now - state.stream_last_render >= state.stream_refresh:
                state.stream_last_render = now
                _render_stream(state, log, segments[-1])
        if len(state.stream_buffer) > 1_000_000:
            state.stream_buffer = state.stream_buffer[-100_000:]
        return

    log.write("RX", data)

    for line in data.splitlines():
        parse_status_line(state, line)


def check_capture_timeout(state: UiState, log: CommLog) -> None:
    if not state.capturing or state.capture_started_at is None:
        return

    elapsed = time.time() - state.capture_started_at
    if elapsed <= CAPTURE_TIMEOUT_S:
        return

    state.capturing = False
    state.parse_errors += 1
    state.status = f"Capture timeout after {elapsed:.1f}s; got {len(state.capture_text)} bytes"
    append_event(state.events, state.status)
    log.event(state.status)
    state.last_line = state.capture_text.splitlines()[-1].strip() if state.capture_text.splitlines() else state.last_line


def _replay_capture_worker(state: UiState, log: CommLog, capture: Capture, realtime: bool) -> None:
    try:
        sent = send_capture_udp(
            capture,
            state.udp_address,
            state.udp_port,
            realtime=realtime,
        )
    except Exception as exc:
        state.status = f"UDP replay failed: {exc}"
        append_event(state.events, state.status)
        log.event(state.status)
        return
    state.udp_packets += sent
    state.status = f"UDP replay sent {sent} packets"
    append_event(state.events, state.status)
    log.event(state.status)


def replay_capture(state: UiState, log: CommLog, realtime: bool = False) -> None:
    if state.last_capture is None:
        append_event(state.events, "no capture to replay")
        log.event("no capture to replay")
        return
    if state.udp_replaying:
        append_event(state.events, "UDP replay already running")
        return
    state.udp_replaying = True
    state.status = "UDP replay running..."
    capture = state.last_capture

    def worker() -> None:
        try:
            _replay_capture_worker(state, log, capture, realtime)
        finally:
            state.udp_replaying = False

    threading.Thread(target=worker, daemon=True, name="udp-replay").start()


def setup_firmware(ser: serial.Serial, state: UiState, log: CommLog, mode: str, hz: int, amp: float) -> None:
    append_event(state.events, "initializing firmware")
    log.event("initializing firmware")
    read_available_logged(ser, log, max_s=0.5)
    for chunk in (send_key_logged(ser, state, log, "q"),):
        for line in chunk.splitlines():
            parse_status_line(state, line)
    if mode == "sine":
        for line in send_key_logged(ser, state, log, "m").splitlines():
            parse_status_line(state, line)
    for chunk in (ramp_amplitude_logged(ser, state, log, amp), ramp_frequency_logged(ser, state, log, hz)):
        for line in chunk.splitlines():
            parse_status_line(state, line)
    append_event(state.events, f"ready mode={state.mode} hz={state.hz} amp={state.amp:g}%")
    log.event(f"ready mode={state.mode} hz={state.hz} amp={state.amp:g}%")


def update_rates(state: UiState, last_rate_at: float) -> float:
    now = time.time()
    if now - last_rate_at < 1.0:
        return last_rate_at
    elapsed = now - last_rate_at
    state.bytes_per_sec = (state.bytes_rx - state.last_bytes_rx) / elapsed
    state.udp_packets_per_sec = (state.udp_packets - state.udp_packets_last) / elapsed
    state.last_bytes_rx = state.bytes_rx
    state.udp_packets_last = state.udp_packets
    return now


def clear_line(row: int, text: str = "") -> None:
    print(term.move_yx(row, 0) + text[: term.width - 1].ljust(term.width - 1), flush=True)


def draw(state: UiState) -> None:
    capture_state = "idle"
    if state.streaming:
        arch = state.stream_archive_path.name if state.stream_archive_path else "?"
        capture_state = f"STREAMING ({state.stream_dumps} dumps -> {arch})"
    elif state.capturing:
        elapsed = time.time() - (state.capture_started_at or time.time())
        capture_state = f"active {elapsed:.1f}s, {len(state.capture_text)} bytes"
    elif state.postprocessing:
        capture_state = "processing snapshot"
    elif state.udp_replaying:
        capture_state = "udp replay running"

    clear_line(1, term.bold("rinz motor scope UI"))
    clear_line(
        3,
        f"UART {state.port}@{state.baud} "
        f"{term.green('connected') if state.connected else term.red('disconnected')} | "
        f"RX {state.bytes_per_sec / 1024:.1f} KiB/s | parse errors {state.parse_errors}",
    )
    clear_line(
        4,
        f"Drive mode={state.mode} hz={state.hz} amp={state.amp:g}% trim={state.trim:+d} | "
        f"capture={capture_state} | {state.rotor_summary}",
    )
    clear_line(
        5,
        f"UDP {state.udp_address}:{state.udp_port} packets={state.udp_packets} "
        f"({state.udp_packets_per_sec:.0f}/s) loop={'on' if state.loop_replay else 'off'}",
    )
    clear_line(6, f"Last capture: {format_stats(state.last_capture)}")
    clear_line(7, f"Status: {state.status}")
    clear_line(8, f"Comms log: {state.comm_log_path if state.comm_log_path else 'disabled'}")
    y_desc = (
        "auto"
        if state.snapshot_ymin is None and state.snapshot_ymax is None
        else f"{state.snapshot_ymin}..{state.snapshot_ymax}"
    )
    clear_line(
        13,
        f"Snapshot: {state.snapshot_path} | avg={state.snapshot_window} | y={y_desc}",
    )

    debug = " ".join(f"{k}={v}" for k, v in state.debug.items())
    clear_line(9, term.bold("Firmware debug:"))
    clear_line(10, debug or "(none)")
    clear_line(11, state.regs[-2] if len(state.regs) >= 2 else "")
    clear_line(12, state.regs[-1] if state.regs else "")

    clear_line(14, term.bold("Controls:"))
    clear_line(15, "F/V hz +/-10 | G/B hz +/-1 | A/Z amp +/-1 | +/- amp +/-0.1 | [/] duty +/-1ct | Q reset | W kill")
    clear_line(16, "D capture | L/K stream start/stop | S log point | U UDP replay | O loop replay | X quit")
    clear_line(17, f"ZC: {state.zc_summary} | {state.zc_path}")

    clear_line(18, term.bold("Events:"))
    events = list(state.events)[-10:]
    for i in range(10):
        event = events[i] if i < len(events) else ""
        clear_line(19 + i, f"  {event}")

    clear_line(30, f"Last line: {state.last_line}")


def handle_key(key: str, ser: serial.Serial, state: UiState, log: CommLog) -> bool:
    k = key.lower()
    if k == "x":
        return False
    if k in ("d", "c") and not state.capturing and not state.postprocessing and not state.streaming:
        start_capture(ser, state, log, key=k)
    elif k in ("d", "c"):
        if state.streaming:
            state.status = "Streaming active; press k to stop before a single capture"
        elif state.capturing:
            state.status = f"Capture already active; got {len(state.capture_text)} bytes"
        elif state.postprocessing:
            state.status = "Capture still processing snapshot"
        else:
            state.status = "Capture unavailable"
        append_event(state.events, state.status)
        log.event(state.status)
    elif k == "l":
        send_raw_key(ser, state, log, "l", "stream start")
        state.streaming = True
        state.stream_buffer = ""
        state.stream_dumps = 0
        stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        state.stream_archive_path = Path("logs") / f"stream_{stamp}.log"
        state.stream_archive_idx = 0
        append_event(state.events, f"stream archive {state.stream_archive_path}")
    elif k == "k":
        send_raw_key(ser, state, log, "k", "stream stop")
        state.streaming = False
    elif k == "s":
        log_exploration_point(state, log)
    elif k in {"g", "b", "[", "]"}:
        # Fine firmware trims: g/b = freq +/-1Hz, ]/[ = duty +/-1 raw CCR count.
        send_raw_key(ser, state, log, k, f"sent {k}")
        if k == "g":
            state.hz += 1
        elif k == "b":
            state.hz = max(1, state.hz - 1)
        elif k == "]":
            state.trim += 1
        elif k == "[":
            state.trim -= 1
    elif k == "u":
        replay_capture(state, log)
    elif k == "o":
        state.loop_replay = not state.loop_replay
        append_event(state.events, f"loop replay {'enabled' if state.loop_replay else 'disabled'}")
        log.event(f"loop replay {'enabled' if state.loop_replay else 'disabled'}")
    elif k == "q":
        send_raw_key(ser, state, log, "q", "reset requested")
        state.mode = "six-step"
        state.hz = 60
        state.amp = 11.0 # firmware AMP_START (the "reset:" echo confirms it)
        state.trim = 0
    elif k in {"f", "v", "a", "z", "+", "-", "m", "w", "y", "0", "1", "2", "3"}:
        send_raw_key(ser, state, log, k, f"sent {k}")
        if k == "f":
            state.hz += 10
        elif k == "v":
            state.hz = max(10, state.hz - 10)
        elif k == "a":
            state.amp += 1.0
        elif k == "z":
            state.amp = max(0.0, state.amp - 1.0)
        elif k == "+":
            state.amp += 0.1
        elif k == "-":
            state.amp = max(0.0, state.amp - 0.1)
        if k == "m":
            state.mode = "sine" if state.mode == "six-step" else "six-step"
        if k == "w":
            # Firmware 'w' clears its STREAMING flag too; keep the UI in sync.
            state.streaming = False
    return True


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Blessed UART UI for rinz motor scope")
    parser.add_argument("port", help="UART port, e.g. COM41")
    parser.add_argument("--baud", type=int, default=BAUD, help=f"UART baud (default: {BAUD})")
    parser.add_argument("--udp-address", default="127.0.0.1", help="UDP address for PlotJuggler")
    parser.add_argument("--udp-port", type=int, default=9870, help="UDP port for PlotJuggler")
    parser.add_argument("--mode", choices=("sine", "six-step"), default="sine")
    parser.add_argument("--hz", type=float, default=120)
    parser.add_argument("--amp", type=float, default=15)
    parser.add_argument("--lowpass-window", type=int, default=9, help="reserved for future UI plot views")
    parser.add_argument("--comm-log", type=Path, default=None, help="host-side TX/RX log path")
    parser.add_argument("--no-comm-log", action="store_true", help="disable host-side TX/RX logging")
    parser.add_argument(
        "--snapshot-path",
        type=Path,
        default=Path("logs/latest_snapshot.png"),
        help="PNG overwritten after every successful capture",
    )
    parser.add_argument(
        "--snapshot-log-path",
        type=Path,
        default=Path("logs/latest_snapshot.log"),
        help="raw capture log overwritten after every successful capture",
    )
    parser.add_argument("--snapshot-window", type=int, default=SNAPSHOT_WINDOW, help="moving-average window for snapshot PNG")
    parser.add_argument("--snapshot-ymin", type=float, default=SNAPSHOT_YMIN, help="snapshot Y-axis minimum (default: auto)")
    parser.add_argument("--snapshot-ymax", type=float, default=SNAPSHOT_YMAX, help="snapshot Y-axis maximum (default: auto)")
    parser.add_argument(
        "--zc-path",
        type=Path,
        default=Path("logs/latest_zc.png"),
        help="zero-crossing PNG overwritten after every six-step capture",
    )
    parser.add_argument(
        "--zc-log-path",
        type=Path,
        default=Path("logs/latest_zc.txt"),
        help="per-sector ZC table overwritten after every six-step capture",
    )
    parser.add_argument(
        "--zc-window",
        type=int,
        default=3,
        help="smoothing window for the ZC plot/detection (default: 3)",
    )
    parser.add_argument(
        "--exploration-path",
        type=Path,
        default=Path("logs/exploration.json"),
        help="JSON file appended with an operating-point record on each 's' keypress",
    )
    parser.add_argument(
        "--archive-dir",
        type=Path,
        default=Path("logs/captures"),
        help="every capture is also copied here as a numbered capNNN_* set",
    )
    parser.add_argument("--no-init", action="store_true", help="do not reset/ramp firmware on startup")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    hz = int(round(args.hz / 10.0) * 10)
    comm_log_path = None
    if not args.no_comm_log:
        stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        comm_log_path = args.comm_log or Path("logs") / f"scope_live_ui_{stamp}.log"

    state = UiState(
        port=args.port,
        baud=args.baud,
        udp_address=args.udp_address,
        udp_port=args.udp_port,
        mode=args.mode,
        hz=hz,
        amp=args.amp,
        comm_log_path=comm_log_path,
        snapshot_path=args.snapshot_path,
        snapshot_log_path=args.snapshot_log_path,
        snapshot_window=args.snapshot_window,
        snapshot_ymin=args.snapshot_ymin,
        snapshot_ymax=args.snapshot_ymax,
        zc_path=args.zc_path,
        zc_log_path=args.zc_log_path,
        zc_window=args.zc_window,
        archive_dir=args.archive_dir,
        exploration_path=args.exploration_path,
    )

    log = CommLog(comm_log_path)
    try:
        with serial.Serial(args.port, args.baud, timeout=0.01) as ser:
            state.connected = True
            ser.reset_input_buffer()
            ser.reset_output_buffer()
            log.event(f"opened serial port={args.port} baud={args.baud}")
            if not args.no_init:
                setup_firmware(ser, state, log, args.mode, hz, args.amp)

            with term.fullscreen(), term.cbreak(), term.hidden_cursor():
                print(term.clear)
                last_draw = 0.0
                last_rate_at = time.time()
                while state.running:
                    poll_serial(ser, state, log)
                    check_capture_timeout(state, log)
                    last_rate_at = update_rates(state, last_rate_at)

                    if (
                        state.loop_replay
                        and state.last_capture
                        and not state.udp_replaying
                        and time.time() - state.last_replay_at > 1.0
                    ):
                        replay_capture(state, log)
                        state.last_replay_at = time.time()

                    key = term.inkey(timeout=0.03)
                    if key:
                        state.running = handle_key(str(key), ser, state, log)

                    if time.time() - last_draw > 0.1:
                        draw(state)
                        last_draw = time.time()
    finally:
        log.close()

    print(term.normal + term.clear + term.move_yx(0, 0) + "scope UI exited")
    return 0


if __name__ == "__main__":
    sys.exit(main())
