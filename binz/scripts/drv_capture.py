#!/usr/bin/env python3
"""Capture or replay a DRV8304 drive plus high-impedance coast trace.

Live mode owns the UART for the whole transaction:

    off -> cap1 -> du<tenths_percent> -> run<target_hz> -> COAST END -> off -> cap0

The firmware records while driving, disables all gates and ENABLE, and only
then emits fixed-width raw hex. This tool preserves that raw dump, validates
its framing, decodes calibrated engineering units to CSV, and renders a plot.
An existing raw dump can be replayed without opening hardware via --infile.
"""

from __future__ import annotations

import argparse
import base64
import struct
import zlib
import csv
import pathlib
import re
import statistics
import time


ROOT = pathlib.Path(__file__).resolve().parent.parent
CAPTURE_DIR = ROOT / "captures"
FIELDS = (
    "tick",
    "freq_chz",
    "ia",
    "ib",
    "ic",
    "vsenc",
    "neutral",
    "vbus",
    "vref",
    "theta",
    "flags",
    "on_a",
    "on_b",
    "on_c",
    "stage",
)
COAST_FIELDS = ("tick", "vsenc", "neutral", "vbus", "vref", "flags")
HEADER_RE = re.compile(
    r"CAP n=(\d+) capacity=(\d+) head=(\d+) order=oldest_first "
    r"sample_hz=(\d+) sample_point=(?:post_pulse_low|control_tick_async_to_pwm) "
    r"adc_order=(?:rotating_ABC_BCA_CAB|DMA_ascending_0_1_4_6_13_logical_4_1_0) "
    r"vcal=(\d+) reason=(\d+) fields=([^\r\n]+)"
)
COAST_HEADER_RE = re.compile(
    r"COAST n=(\d+) sample_hz=(\d+) sample_point=bridge_disabled "
    r"vcal=(\d+) fields=([^\r\n]+)"
)
ROW_RE = re.compile(r"^[0-9a-fA-F]{4}(?: [0-9a-fA-F]{4}){14}$")
COAST_ROW_RE = re.compile(r"^[0-9a-fA-F]{4}(?: [0-9a-fA-F]{4}){5}$")


def send_line(port, line: str) -> None:
    port.write((line + "\r\n").encode("ascii"))
    port.flush()


def send_live_line(port, line: str) -> None:
    # Same bounded byte pacing as live frequency updates: the firmware's
    # single-byte RDR can overrun during foreground ADC work.
    for byte in (line + '\r').encode('ascii'):
        port.write(bytes([byte]))
        port.flush()
        time.sleep(.003)


def read_available(port, seconds: float) -> bytes:
    deadline = time.monotonic() + seconds
    data = bytearray()
    while time.monotonic() < deadline:
        block = port.read(65536)
        if block:
            data.extend(block)
        else:
            time.sleep(0.01)
    return bytes(data)


def verify_off(reply: bytes) -> None:
    out = re.search(rb'OUT: ([^\r\n]+)', reply)
    inp = re.search(rb'IN: ([^\r\n]+)', reply)
    gates = [b'ah=0', b'bh=0', b'ch=0', b'al=0', b'bl=0', b'cl=0', b'en=0']
    if (not out or not inp or
            not all(v in out[1].split() and v in inp[1].split() for v in gates) or
            not all(v in out[1].split() for v in [b'TIM1:moe=0', b'ccrA=0', b'ccrB=0', b'ccrC=0']) or
            b'nflt=1' not in inp[1].split()):
        raise RuntimeError('output-off/nFAULT readback failed; stop campaign')


def step_ack(reply: bytes, target: int) -> bool:
    if b'!step' in reply or b'!hz' in reply or b'!busy' in reply:
        raise RuntimeError(f'firmware rejected startup frequency step {target}')
    return re.search(rb'(?:^|\n)F'+str(target).encode()+rb'\r?\n',reply) is not None


def live_capture(port_name: str, baud: int, target_hz: int, duty_tenths: int, fixed: bool, duration: float | None, observe: bool = False, raw_path: pathlib.Path | None = None, step_targets: tuple[int, ...] = (), observe_duty: int = 0, catch_duty: int | None = None, engage_duty: int | None = None, engage_ms: int = 1000, core_trace: int | None = None, seed_sector: int | None = None, capture_stride: int | None = None, dropout: bool = False, reentry: bool = False, driven: bool = False, drive_sample: int = 192, drive_duty: int = 0, drive_phase: int = 0, drive_handoff: bool = False, bemf_duty: int | None = None, live_poll=None, step_start_s: float = 3.2, step_span_s: float = 1.2, dropout_ms: int = 2000, tracking_trip: bool = False) -> str:
    if not 0.1<=step_start_s<=3.2 or not 0.5<=step_span_s<=4.0 or step_start_s+step_span_s>4.5:
        raise ValueError('startup ramp must finish before the4.7s handoff')
    import serial

    if driven and (observe or engage_duty is not None or fixed or (dropout and not drive_handoff) or (reentry and not (drive_handoff and dropout))):
        raise ValueError('driven observation must not overlap another drive mode')
    if drive_handoff and (not driven or not 20<=engage_ms<=600000):
        raise ValueError('driven handoff requires driven mode and a finite 20..600000 ms window')
    if not 2000<=dropout_ms<=10000:
        raise ValueError('dropout schedule must be2000..10000ms')
    if tracking_trip and (not drive_handoff or not driven or dropout):
        raise ValueError('tracking trip requires driven handoff and is exclusive with dropout suppression')
    if drive_handoff and (dropout or tracking_trip) and engage_ms<=dropout_ms:
        raise ValueError('driven fault injection requires a window longer than its schedule')
    port = serial.Serial(port_name, baud, timeout=0.05)
    data = bytearray()
    try:
        port.reset_input_buffer()
        send_line(port, "off")
        data.extend(read_available(port, 0.15))
        if driven:
            state=bytearray()
            for cmd in ['engage0','obs0','p','i']:
                send_line(port,cmd);state.extend(read_available(port,.15))
            data.extend(state);verify_off(state)
            if drive_phase not in (-30,0,30,60): raise ValueError('unqualified phase experiment')
            send_line(port,f'drivephase{drive_phase}');ack=read_available(port,.15);data.extend(ack)
            if f'DRIVEPHASE target={drive_phase} accepted=1 one_shot=1 gate_authority=0'.encode() not in ack:
                raise RuntimeError('missing driven phase acknowledgement')
            if drive_duty!=0 and not 40<=drive_duty<=100: raise ValueError('driven startup duty must be4..10 percent; higher settings are exploratory')
            send_line(port,f'drivedu{drive_duty}');ack=read_available(port,.15);data.extend(ack)
            if f'DRIVEDUTY target={drive_duty} accepted=1 one_shot=1 gate_authority=0'.encode() not in ack:
                raise RuntimeError('missing driven duty acknowledgement')
            if bemf_duty is not None:
                if not drive_handoff or not 40<=bemf_duty<=300:
                    raise ValueError('BEMF duty requires handoff and 4..30% carrier range')
                send_line(port,f'bemfdu{bemf_duty}');ack=read_available(port,.15);data.extend(ack)
                if f'BEMFDUTY target={bemf_duty} accepted=1 one_shot=1 gate_authority=0'.encode() not in ack:
                    raise RuntimeError('missing independent BEMF duty acknowledgement')
            if drive_sample not in (192,320): raise ValueError('unqualified PWM sample target')
            send_line(port,f'drivepwm{drive_sample}');ack=read_available(port,.15);data.extend(ack)
            if f'DRIVEPWM target={drive_sample} accepted=1 gate_authority=0'.encode() not in ack:
                raise RuntimeError('missing PWM sample acknowledgement')
            send_line(port,'driveobs1');ack=read_available(port,.15);data.extend(ack)
            if b'DRIVEOBS armed=1 window_us=20000 handoff_authority=0 one_shot=1' not in ack:
                raise RuntimeError('missing guarded under-drive mode acknowledgement')
            if drive_handoff:
                for cmd,expected in [(f'engagems{engage_ms}',f'ENGAGEWINDOW ms={engage_ms}'),
                                     ('drivex1','DRIVEX armed=1 accepted=1 one_shot=1')]:
                    send_line(port,cmd);ack=read_available(port,.15);data.extend(ack)
                    if expected.encode() not in ack:
                        raise RuntimeError('missing driven handoff handshake')
        if dropout or tracking_trip:
            if drive_handoff:
                send_line(port,f'drivedropms{dropout_ms}');ack=read_available(port,.15);data.extend(ack)
                if f'DRIVEDROPWINDOW ms={dropout_ms} range=2000..10000 idle_only=1'.encode() not in ack:
                    raise RuntimeError('missing driven dropout window handshake')
        if tracking_trip:
            send_line(port,'drivetrack1');ack=read_available(port,.15);data.extend(ack)
            if b'DRIVETRACK armed=1 configured_window=1 immediate_trip=1 one_shot=1' not in ack:
                raise RuntimeError('missing driven tracking-trip handshake')
        if dropout:
            send_line(port,'drivedrop1' if drive_handoff else 'dropout1')
            ack=read_available(port,.15);data.extend(ack)
            expected=b'DRIVEDROP armed=1 configured_window=1 stop_only=1 one_shot=1' if drive_handoff else b'DROPOUT armed=1 after_ms=2000 stop_only=1'
            if expected not in ack:
                raise RuntimeError('missing dropout injection handshake')
        if reentry:
            send_line(port,'drivereentry1' if drive_handoff else 'reentry1');ack=read_available(port,.15);data.extend(ack)
            expected=b'DRIVEREENTRY armed=1 tracking_only=1 attempts_max=1 one_shot=1' if drive_handoff else b'REENTRY armed=1 tracking_only=1 attempts_max=1'
            if expected not in ack:
                raise RuntimeError('missing guarded reentry handshake')
        if core_trace is not None:
            send_line(port,f'coretrace{core_trace}')
            ack=read_available(port,.15);data.extend(ack)
            if f'CORETRACE enabled={core_trace}'.encode() not in ack:
                raise RuntimeError('missing IRQ trace mode handshake')
        if seed_sector is not None:
            send_line(port,f'flyseed{seed_sector}')
            ack=read_available(port,.15);data.extend(ack)
            if f'FLYSEED sector={seed_sector} zero_any=1'.encode() not in ack:
                raise RuntimeError('missing measured seed sector handshake')
        if engage_duty is not None:
            state=bytearray()
            for cmd in ['p','i']:
                send_line(port,cmd);state.extend(read_available(port,.15))
            data.extend(state);verify_off(state)
            for cmd,expected in [(f'engagems{engage_ms}',f'ENGAGEWINDOW ms={engage_ms}'),
                                 (f'engagedu{engage_duty}',f'ENGAGEDUTY override_tenths={engage_duty} '),
                                 ('engage1','ENGAGE armed=1 clears_other_probes=1 gate_authority=1')]:
                send_line(port,cmd);ack=read_available(port,.15);data.extend(ack)
                if expected.encode() not in ack: raise RuntimeError('missing powered handoff handshake')
        if capture_stride is not None:
            send_line(port,f'capstride{capture_stride}')
            ack=read_available(port,.15);data.extend(ack)
            if f'CAPSTRIDE ticks={capture_stride} adc_hz=1000 guards_unchanged=1'.encode() not in ack:
                raise RuntimeError('missing capture stride handshake')
        send_line(port, "cap1")
        handshake = read_available(port, 0.15)
        data.extend(handshake)
        if b"CAPTURE armed one-shot a85-v1" not in handshake:
            raise RuntimeError("firmware lacks capture opt-in handshake; update shell-pwm before live capture")
        send_line(port, f"du{duty_tenths}")
        data.extend(read_available(port, 0.15))
        if fixed:
            send_line(port, f"sf{target_hz}")
            data.extend(read_available(port, 0.15))
            send_line(port, "sine")
        else:
            if catch_duty is not None:
                send_line(port, f'catchdu{catch_duty}')
                ack=read_available(port,.15);data.extend(ack)
                if f'CATCHDUTY tenths={catch_duty}'.encode() not in ack:
                    raise RuntimeError('missing catch duty handshake')
            if observe:
                send_line(port,f'obsdu{observe_duty}')
                duty_ack=read_available(port,.15);data.extend(duty_ack)
                if f'OBSDUTY armed={observe_duty}'.encode() not in duty_ack:
                    raise RuntimeError('missing observation duty handshake')
                send_line(port, 'obs1')
                ack = read_available(port, 0.15)
                data.extend(ack)
                if b'OBS armed one-shot' not in ack:
                    raise RuntimeError('missing observation handshake')
            send_line(port, f"run{target_hz}")

        run_started = time.monotonic()
        stop_sent = False
        step_index = 0
        pending_step = None
        step_reply_start = 0
        step_sent_at = 0.0
        deadline = run_started + 15.0 + (engage_ms/1000 if engage_duty is not None or drive_handoff else 0)
        while time.monotonic() < deadline:
            block = port.read(65536)
            if block:
                data.extend(block)
                if b'RUN refused: prestart baseline incomplete; gates + en OFF' in data:
                    # Retain the following partial BZ85 evidence; do not send
                    # speed-ramp commands to an attempt that never started.
                    data.extend(read_available(port,.3))
                    raise RuntimeError('prestart baseline refused; partial evidence retained')
                # Compact lean images suppress the bulk coast dump and end
                # with DONE instead. Requiring COAST END there turns a clean
                # protected hold into a false host timeout.
                if b"COAST END" in data or (
                    b"LEANCORE r1 recorder=0" in data
                    and re.search(rb"(?:^|\n)DONE reason=\d+", data)
                ):
                    break
            if live_poll is not None:
                live_poll(port,time.monotonic()-run_started,data)
            if pending_step is not None:
                if step_ack(bytes(data[step_reply_start:]),pending_step):
                    pending_step=None
                elif b'TIMING energized_us=' not in data and time.monotonic()-step_sent_at>0.5:
                    raise RuntimeError('startup frequency step acknowledgement timed out')
            if duration is not None and not stop_sent and time.monotonic() - run_started >= duration:
                send_live_line(port, "off")
                stop_sent = True
            if (pending_step is None and step_index < len(step_targets) and b'TIMING energized_us=' not in data
                    and time.monotonic()-run_started >= step_start_s+(step_span_s/max(1,len(step_targets)-1))*step_index):
                # Foreground UART servicing shares time with ADC scans. Pace
                # live commands so a burst cannot overrun the single-byte RDR.
                pending_step=step_targets[step_index]
                step_reply_start=len(data)
                step_sent_at=time.monotonic()
                send_live_line(port, f'ehz{pending_step}')
                step_index += 1
        if b"COAST END" not in data and not (
            b"LEANCORE r1 recorder=0" in data
            and re.search(rb"(?:^|\n)DONE reason=\d+", data)
        ):
            raise RuntimeError("capture timed out before COAST END or compact DONE")
    finally:
        # Host-side kill guard. Firmware already disables before dumping, but
        # every host exit path independently requests the same safe state.
        try:
            send_live_line(port, "")  # terminate any partial command first
            send_live_line(port, "off")
            read_available(port, 0.15)
            send_line(port, "cap0")
            read_available(port, 0.15)
            if catch_duty is not None:
                send_line(port, 'catchdu70')
                read_available(port, .15)
            if observe:
                send_line(port,'obsdu0')
                read_available(port,.15)
                send_line(port, 'obs0')
                read_available(port, 0.15)
            if engage_duty is not None:
                state=bytearray()
                for cmd in ['engage0','engagedu0','engagems1000','p','i']:
                    send_line(port,cmd);state.extend(read_available(port,.15))
                data.extend(state);verify_off(state)
            if capture_stride is not None:
                send_line(port,'capstride1');data.extend(read_available(port,.15))
            if core_trace is not None:
                send_line(port,'coretrace0');data.extend(read_available(port,.15))
            if dropout:
                send_line(port,'drivedrop0' if drive_handoff else 'dropout0');data.extend(read_available(port,.15))
            if tracking_trip:
                send_line(port,'drivetrack0');data.extend(read_available(port,.15))
            if drive_handoff and (dropout or tracking_trip):
                send_line(port,'drivedropms2000');data.extend(read_available(port,.15))
            if reentry:
                send_line(port,'drivereentry0' if drive_handoff else 'reentry0');data.extend(read_available(port,.15))
            if seed_sector is not None:
                send_line(port,'flyseed0');data.extend(read_available(port,.15))
            if driven:
                state=bytearray(b'FINALOFF\n')
                reset_transfer=['drivex0','engagems1000'] if drive_handoff else []
                if bemf_duty is not None:reset_transfer.append('bemfdu0')
                for cmd in ['obs0',*reset_transfer,'drivephase0','drivedu0','drivepwm192','p','i','stack']:
                    send_line(port,cmd);state.extend(read_available(port,.15))
                data.extend(state);verify_off(state)
        finally:
            port.close()
            # Preserve failure evidence too; do not require successful parsing
            # or COAST END before saving bytes already received.
            if raw_path is not None:
                raw_path.write_bytes(data)
    return data.decode("ascii", errors="replace")


def expand_snapshot(text: str) -> str:
    """Validate compact records, then reuse the historical raw-field parser."""
    if "WIRE a85-v1" not in text:
        if re.search(r"^(?:D85|C85|WIRE) ", text, re.MULTILINE):
            raise RuntimeError("missing or unsupported wire version")
        return text
    lines = []
    section = None
    coast_index = 0
    for line in text.splitlines():
        if line.startswith("CAP n="):
            section = "D85"
        elif line.startswith("COAST n="):
            section = "C85"
        elif line in ("CAP END", "COAST END"):
            section = None
        elif line.startswith(("D85 ", "C85 ")):
            kind, body = line.split(" ", 1)
            if kind != section:
                raise RuntimeError("record in wrong snapshot section")
            try:
                raw = base64.a85decode(body.encode("ascii"))
            except (ValueError, UnicodeError) as exc:
                raise RuntimeError("invalid Ascii85 record") from exc
            expected = 34 if kind == "D85" else 20
            if len(raw) != expected:
                raise RuntimeError("invalid snapshot record length")
            if zlib.crc32(raw[:-4]) != struct.unpack("<I", raw[-4:])[0]:
                raise RuntimeError("snapshot CRC mismatch")
            values = struct.unpack("<15H" if kind == "D85" else "<8H", raw[:-4])
            lines.append(" ".join(f"{v:04x}" for v in values[:15 if kind == "D85" else 6]))
            if kind == "C85":
                if values[0] != coast_index:
                    raise RuntimeError("unordered coast records")
                lines.append(f"CTIME {coast_index} {values[6] | (values[7] << 16)}")
                coast_index += 1
            continue
        elif section and line.strip():
            raise RuntimeError("unexpected data in compact snapshot")
        lines.append(line)
    return "\n".join(lines)


def parse_dump(text: str):
    text = expand_snapshot(text)
    match = HEADER_RE.search(text)
    if not match:
        raise RuntimeError("no CAP header found")
    n, capacity, head, sample_hz, vcal, reason = map(int, match.group(1, 2, 3, 4, 5, 6))
    fields = tuple(match.group(7).strip().split(","))
    if fields != FIELDS:
        raise RuntimeError(f"field contract mismatch: {fields!r}")
    if n > capacity or head >= capacity:
        raise RuntimeError(f"invalid ring metadata: n={n} capacity={capacity} head={head}")
    end = text.find("CAP END", match.end())
    if end < 0:
        raise RuntimeError("CAP END missing")
    rows = []
    for line in text[match.end() : end].splitlines():
        line = line.strip()
        if ROW_RE.fullmatch(line):
            values = tuple(int(word, 16) for word in line.split())
            rows.append(dict(zip(FIELDS, values)))
    if len(rows) != n:
        raise RuntimeError(f"short or malformed capture: {len(rows)}/{n} records")
    if any(b['tick'] <= a['tick'] for a, b in zip(rows, rows[1:])):
        raise RuntimeError("unordered drive records")
    coast_match = COAST_HEADER_RE.search(text, end)
    if not coast_match:
        raise RuntimeError("COAST header missing")
    coast_n, coast_hz, coast_vcal = map(int, coast_match.group(1, 2, 3))
    coast_fields = tuple(coast_match.group(4).strip().split(","))
    if coast_fields != COAST_FIELDS:
        raise RuntimeError(f"coast field contract mismatch: {coast_fields!r}")
    if coast_vcal != vcal:
        raise RuntimeError(f"VREF calibration mismatch: drive={vcal} coast={coast_vcal}")
    coast_end = text.find("COAST END", coast_match.end())
    if coast_end < 0:
        raise RuntimeError("COAST END missing")
    coast_rows = []
    for line in text[coast_match.end() : coast_end].splitlines():
        line = line.strip()
        if COAST_ROW_RE.fullmatch(line):
            values = tuple(int(word, 16) for word in line.split())
            coast_rows.append(dict(zip(COAST_FIELDS, values)))
    if len(coast_rows) != coast_n:
        raise RuntimeError(f"short or malformed coast capture: {len(coast_rows)}/{coast_n} records")
    stamps = re.findall(r"^CTIME (\d+) (\d+)\s*$", text[coast_match.end():coast_end], re.MULTILINE)
    if stamps:
        if len(stamps) != coast_n or [int(i) for i, _ in stamps] != list(range(coast_n)):
            raise RuntimeError("incomplete or unordered CTIME records")
        for row, (_, stamp) in zip(coast_rows, stamps):
            row["elapsed_us"] = int(stamp)
        if any(b['elapsed_us'] <= a['elapsed_us'] for a, b in zip(coast_rows, coast_rows[1:])):
            raise RuntimeError("nonmonotonic coast timestamps")
    return rows, sample_hz, coast_rows, coast_hz, vcal, reason


def scale_rows(rows, vcal: int):
    scaled = []
    for row in rows:
        vref = row["vref"]
        vdda_mv = 3000.0 * vcal / vref if vref else float("nan")

        def mv(name: str) -> float:
            return row[name] * vdda_mv / 4096.0

        offset_mv = vdda_mv / 2.0
        out = dict(row)
        out.update(
            time_ms=float(row["tick"]),
            frequency_hz=row["freq_chz"] / 100.0,
            vdda_mv=vdda_mv,
            ia_ma=(mv("ia") - offset_mv) * 1000.0 / 70.0,
            ib_ma=(mv("ib") - offset_mv) * 1000.0 / 70.0,
            ic_ma=(mv("ic") - offset_mv) * 1000.0 / 70.0,
            vsenc_mv=mv("vsenc"),
            neutral_mv=mv("neutral"),
            vbus_mv=mv("vbus") * 11.94,
            no_fault=bool(row["flags"] & 1),
            comp_a=bool(row["flags"] & 2),
            comp_b=bool(row["flags"] & 4),
            comp_c=bool(row["flags"] & 8),
        )
        scaled.append(out)
    return scaled


def scale_coast(rows, vcal: int, sample_hz: int):
    scaled = []
    for row in rows:
        vref = row["vref"]
        vdda_mv = 3000.0 * vcal / vref if vref else float("nan")

        def mv(name: str) -> float:
            return row[name] * vdda_mv / 4096.0

        out = dict(row)
        out.update(
            time_ms=row.get("elapsed_us", float(row["tick"]) * 1_000_000.0 / sample_hz) / 1000.0,
            vdda_mv=vdda_mv,
            vsenc_mv=mv("vsenc"),
            neutral_mv=mv("neutral"),
            vbus_mv=mv("vbus") * 11.94,
            no_fault=bool(row["flags"] & 1),
            comp_a=bool(row["flags"] & 2),
            comp_b=bool(row["flags"] & 4),
            comp_c=bool(row["flags"] & 8),
        )
        scaled.append(out)
    return scaled


def write_csv(path: pathlib.Path, rows) -> None:
    columns = list(FIELDS) + [
        "time_ms",
        "frequency_hz",
        "vdda_mv",
        "ia_ma",
        "ib_ma",
        "ic_ma",
        "vsenc_mv",
        "neutral_mv",
        "vbus_mv",
        "no_fault",
        "comp_a",
        "comp_b",
        "comp_c",
    ]
    with path.open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=columns)
        writer.writeheader()
        writer.writerows(rows)


def write_coast_csv(path: pathlib.Path, rows) -> None:
    columns = list(COAST_FIELDS) + ["elapsed_us"] + [
        "time_ms",
        "vdda_mv",
        "vsenc_mv",
        "neutral_mv",
        "vbus_mv",
        "no_fault",
        "comp_a",
        "comp_b",
        "comp_c",
    ]
    with path.open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=columns)
        writer.writeheader()
        writer.writerows(rows)


def comparator_analysis(rows, sample_hz: int, window_ms: float = 80.0):
    import numpy as np

    rows = [row for row in rows if row["time_ms"] <= window_ms]
    phase_events = []
    phase_stats = {}
    debounce_samples = 3
    for phase in ("a", "b", "c"):
        key = f"comp_{phase}"
        events = []
        stable = bool(rows[0][key]) if rows else False
        candidate = stable
        count = 0
        for index, row in enumerate(rows[1:], start=1):
            value = bool(row[key])
            if value == stable:
                candidate = stable
                count = 0
                continue
            if value != candidate:
                candidate = value
                count = 1
            else:
                count += 1
            if count >= debounce_samples:
                event_time = rows[index - debounce_samples + 1]["time_ms"] / 1000.0
                stable = candidate
                count = 0
                events.append(event_time)
                phase_events.append((event_time, phase.upper(), int(stable)))
        intervals = [events[i] - events[i - 1] for i in range(1, len(events))]
        frequency = 1.0 / (2.0 * statistics.median(intervals)) if intervals else None
        phase_stats[phase.upper()] = {"edges": len(events), "frequency_hz": frequency}

    phase_events.sort()
    labels = [event[1] for event in phase_events]
    orders = (("A", "B", "C"), ("A", "C", "B"))
    best_order = None
    best_score = 0.0
    if len(labels) >= 2:
        for order in orders:
            next_phase = {order[i]: order[(i + 1) % 3] for i in range(3)}
            score = sum(labels[i] == next_phase[labels[i - 1]] for i in range(1, len(labels)))
            ratio = score / (len(labels) - 1)
            if ratio > best_score:
                best_score = ratio
                best_order = "->".join(order)
    frequencies = [s["frequency_hz"] for s in phase_stats.values() if s["frequency_hz"]]
    median_frequency = statistics.median(frequencies) if frequencies else None
    initial_frequencies = []
    for phase in ("A", "B", "C"):
        phase_times = [time_s for time_s, event_phase, _ in phase_events if event_phase == phase]
        if len(phase_times) >= 2 and phase_times[1] > phase_times[0]:
            initial_frequencies.append(1.0 / (2.0 * (phase_times[1] - phase_times[0])))
    initial_frequency = statistics.median(initial_frequencies) if initial_frequencies else None
    coherent_events = []
    if best_order:
        order_parts = best_order.split("->")
        next_phase = {order_parts[i]: order_parts[(i + 1) % 3] for i in range(3)}
        for event in phase_events:
            if not coherent_events or event[1] == next_phase[coherent_events[-1][1]]:
                coherent_events.append(event)
            else:
                break
    disable_frequency = None
    phase_fit_r2 = None
    if len(coherent_events) >= 6:
        event_time = np.array([event[0] for event in coherent_events])
        electrical_cycles = np.arange(len(coherent_events)) / 6.0
        fit = np.polyfit(event_time, electrical_cycles, 2)
        predicted = np.polyval(fit, event_time)
        residual = float(np.sum((electrical_cycles - predicted) ** 2))
        total = float(np.sum((electrical_cycles - np.mean(electrical_cycles)) ** 2))
        disable_frequency = float(fit[1])
        phase_fit_r2 = 1.0 - residual / total if total else None
    c_delta = [row["vsenc_mv"] - row["neutral_mv"] for row in rows]
    c_span_mv = max(c_delta) - min(c_delta) if c_delta else 0.0
    c_high = [delta for delta, row in zip(c_delta, rows) if row["comp_c"]]
    c_low = [delta for delta, row in zip(c_delta, rows) if not row["comp_c"]]
    c_level_delta_mv = (
        statistics.mean(c_high) - statistics.mean(c_low) if c_high and c_low else 0.0
    )
    enough_edges = all(values["edges"] >= 3 for values in phase_stats.values())
    motion_valid = bool(
        median_frequency is not None
        and enough_edges
        and best_score >= 0.70
        and c_span_mv >= 6.0
        and abs(c_level_delta_mv) >= 2.0
        and disable_frequency is not None
        and phase_fit_r2 is not None
        and phase_fit_r2 >= 0.98
    )
    return (
        phase_stats,
        phase_events,
        best_order,
        best_score,
        median_frequency,
        initial_frequency,
        disable_frequency,
        phase_fit_r2,
        c_span_mv,
        c_level_delta_mv,
        motion_valid,
    )


def render(path: pathlib.Path, rows, coast_rows, sample_hz: int, coast_hz: int, reason: int) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    t = [row["time_ms"] / 1000.0 for row in rows]
    fig, axes = plt.subplots(5, 1, figsize=(14, 13))

    axes[0].plot(t, [r["ia_ma"] for r in rows], label="IA")
    axes[0].plot(t, [r["ib_ma"] for r in rows], label="IB")
    axes[0].plot(t, [r["ic_ma"] for r in rows], label="IC")
    axes[0].set_ylabel("current (mA)")
    axes[0].legend(ncol=3)

    axes[1].plot(t, [r["vbus_mv"] / 1000.0 for r in rows], label="VBUS")
    axes[1].set_ylabel("bus (V)")
    axes[1].legend()

    axes[2].plot(t, [r["vsenc_mv"] for r in rows], label="VSENC")
    axes[2].plot(t, [r["neutral_mv"] for r in rows], label="neutral")
    axes[2].set_ylabel("feedback (mV)")
    axes[2].legend()

    axes[3].plot(t, [r["on_a"] for r in rows], label="A on")
    axes[3].plot(t, [r["on_b"] for r in rows], label="B on")
    axes[3].plot(t, [r["on_c"] for r in rows], label="C on")
    axes[3].set_ylabel("on-time (us)")
    axes[3].set_xlabel("time (s)")
    axes[3].legend(ncol=3)

    coast_t = [row["time_ms"] / 1000.0 for row in coast_rows]
    axes[4].plot(coast_t, [r["vsenc_mv"] - r["neutral_mv"] for r in coast_rows], label="C-neutral (mV)")
    axes[4].step(coast_t, [500.0 * (int(r["comp_a"]) - 1) for r in coast_rows], where="post", label="A comp")
    axes[4].step(coast_t, [500.0 * int(r["comp_b"]) for r in coast_rows], where="post", label="B comp")
    axes[4].step(coast_t, [500.0 * (int(r["comp_c"]) + 1) for r in coast_rows], where="post", label="C comp")
    axes[4].set_ylabel("coast BEMF")
    axes[4].set_xlabel("time after disable (s)")
    axes[4].legend(ncol=4)

    fault_count = sum(not r["no_fault"] for r in rows)
    fig.suptitle(
        f"DRV8304 capture: {len(rows)} records @ {sample_hz} Hz, "
        f"reason={reason}, fault_samples={fault_count}, coast={len(coast_rows)} @ {coast_hz} Hz"
    )
    for axis in axes:
        axis.grid(alpha=0.25)
    fig.tight_layout()
    fig.savefig(path, dpi=120)


def summarize(rows, coast_rows, sample_hz: int, coast_hz: int, reason: int) -> None:
    print(f"records={len(rows)} sample_hz={sample_hz} reason={reason}")
    for name in ("ia_ma", "ib_ma", "ic_ma", "vbus_mv", "vsenc_mv", "neutral_mv"):
        values = [row[name] for row in rows]
        print(f"{name}: min={min(values):.1f} max={max(values):.1f}")
    print(f"fault_samples={sum(not row['no_fault'] for row in rows)}")
    (
        stats,
        events,
        order,
        order_score,
        median_frequency,
        initial_frequency,
        disable_frequency,
        phase_fit_r2,
        c_span_mv,
        c_level_delta_mv,
        motion_valid,
    ) = comparator_analysis(coast_rows, coast_hz)
    print(f"coast_records={len(coast_rows)} coast_sample_hz={coast_hz}")
    for phase, values in stats.items():
        frequency = values["frequency_hz"]
        text = f"{frequency:.2f}Hz" if frequency is not None else "n/a"
        print(f"coast_comp_{phase}: edges={values['edges']} frequency={text}")
    measured = f"{median_frequency:.2f}Hz" if median_frequency is not None else "n/a"
    initial = f"{initial_frequency:.2f}Hz" if initial_frequency is not None else "n/a"
    disable = f"{disable_frequency:.2f}Hz" if disable_frequency is not None else "n/a"
    fit_quality = f"{phase_fit_r2:.5f}" if phase_fit_r2 is not None else "n/a"
    print(f"coast_measured_electrical_frequency={measured}")
    print(f"coast_initial_electrical_frequency={initial}")
    print(f"coast_disable_extrapolated_frequency={disable} fit_r2={fit_quality}")
    print(f"coast_target_45_55_pass={disable_frequency is not None and 45.0 <= disable_frequency <= 55.0}")
    print(f"coast_phase_order={order or 'n/a'} score={order_score:.3f}")
    print(f"coast_c_minus_neutral_span={c_span_mv:.1f}mV")
    print(f"coast_c_level_delta={c_level_delta_mv:.1f}mV")
    print(f"coast_motion_valid={motion_valid}")
    print("coast_first_edges=" + " ".join(f"{t:.4f}:{p}{level}" for t, p, level in events[:18]))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", default="COM41")
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument("--target", "--freq", dest="target", type=int, default=50,
                        help="campaign target electrical frequency, 1..250 Hz")
    parser.add_argument("--duty-tenths", "--max-on-us", dest="duty_tenths", type=int, default=60,
                        help="peak duty in tenths of a percent, 1..100 (default 60 = 6%%)")
    parser.add_argument("--fixed", action="store_true",
                        help="run fixed-frequency sine diagnostic instead of run<hz> campaign")
    parser.add_argument('--observe', action='store_true', help='append bounded six-step observation at selected target')
    parser.add_argument('--engage-duty-tenths',type=int,help='explicit powered reference handoff duty1..100; verify off before/after and reset override')
    parser.add_argument('--engage-ms',type=int,default=1000,help='powered reference duration20..600000 ms, default1000; startup is additional')
    parser.add_argument('--core-trace',type=int,choices=(0,1),help='explicit per-read IRQ instrumentation A/B mode; disable after capture')
    parser.add_argument('--seed-sector',type=int,choices=range(7),help='diagnostic measured seed sector0..6,0 any; restore0 afterward')
    parser.add_argument('--catch-duty-tenths',type=int,help='campaign catch duty,1..100; restore default70 afterward')
    parser.add_argument('--observe-duty-tenths',type=int,default=0,help='one-shot observation duty,0 inherits run duty,max100')
    parser.add_argument("--duration", type=float,
                        help="host-stop a fixed or campaign run after this many seconds, then capture coast")
    parser.add_argument("--tag", default=time.strftime("drv_%Y%m%d_%H%M%S"))
    parser.add_argument("--infile", type=pathlib.Path)
    parser.add_argument('--step-targets', default='', help='Hold sweep, max15 comma-separated +/-10eHz steps over1.2s; unchanged duty')
    parser.add_argument('--capture-stride',type=int,choices=range(1,101),help='retain every Nth control sample; ADC and guards remain1kHz;19 covers startup')
    parser.add_argument('--dropout',action='store_true',help='suppress BEMF at2s powered; stop-only diagnostic, no restart')
    parser.add_argument('--reentry',action='store_true',help='one guarded reentry after injected tracking loss; original deadline retained')
    args = parser.parse_args()
    if args.reentry and not args.dropout: parser.error('--reentry requires --dropout')
    if args.dropout and (args.engage_duty_tenths is None or args.engage_ms<=2000):
        parser.error('--dropout requires powered handoff window longer than2000ms')
    if not 20<=args.engage_ms<=600000:
        parser.error('--engage-ms must be in20..600000')
    if args.engage_duty_tenths is not None and (args.fixed or args.observe or not 1<=args.engage_duty_tenths<=100):
        parser.error('powered handoff requires campaign, no --observe, duty1..100')
    if args.catch_duty_tenths is not None and (args.fixed or not 1<=args.catch_duty_tenths<=100):
        parser.error('catch duty requires campaign and range1..100')
    if not 0<=args.observe_duty_tenths<=100 or (args.observe_duty_tenths and not args.observe):
        parser.error('observation duty requires --observe and range1..100')
    steps=tuple(int(v) for v in args.step_targets.split(',') if v)
    if len(steps)>15 or (steps and args.fixed):
        parser.error('steps require campaign without --fixed, maximum15')
    previous=args.target
    for hz in steps:
        if not 1<=hz<=250 or abs(hz-previous)>10:
            parser.error('each step must be within10eHz and range1..250')
        previous=hz

    CAPTURE_DIR.mkdir(exist_ok=True)
    if args.infile:
        text = args.infile.read_text(encoding="ascii", errors="replace")
    else:
        if not 1 <= args.duty_tenths <= 100:
            parser.error("--duty-tenths must be in 1..100")
        duration_max=5.0+(args.engage_ms/1000 if args.engage_duty_tenths is not None else 0)
        if args.duration is not None and not 0.1 <= args.duration <= duration_max:
            parser.error(f"--duration must be in 0.1..{duration_max} seconds")
        target_limit = 500 if args.fixed else 250
        if not 1 <= args.target <= target_limit:
            parser.error(f"--target must be in 1..{target_limit}")
        if args.observe and args.fixed:
            parser.error('--observe requires campaign mode')
        text = live_capture(args.port, args.baud, args.target, args.duty_tenths, args.fixed, args.duration, args.observe, CAPTURE_DIR / f"{args.tag}.txt", steps,args.observe_duty_tenths,args.catch_duty_tenths,args.engage_duty_tenths,args.engage_ms,args.core_trace,args.seed_sector,args.capture_stride,args.dropout,args.reentry)

    rows, sample_hz, coast_rows, coast_hz, vcal, reason = parse_dump(text)
    scaled = scale_rows(rows, vcal)
    coast_scaled = scale_coast(coast_rows, vcal, coast_hz)
    raw_path = CAPTURE_DIR / f"{args.tag}.txt"
    csv_path = CAPTURE_DIR / f"{args.tag}.csv"
    coast_csv_path = CAPTURE_DIR / f"{args.tag}_coast.csv"
    png_path = CAPTURE_DIR / f"{args.tag}.png"
    if not args.infile:
        raw_path.write_text(text, encoding="ascii")
        print(f"saved {raw_path}")
    write_csv(csv_path, scaled)
    write_coast_csv(coast_csv_path, coast_scaled)
    render(png_path, scaled, coast_scaled, sample_hz, coast_hz, reason)
    print(f"saved {csv_path}")
    print(f"saved {coast_csv_path}")
    print(f"saved {png_path}")
    if 'RECOVERYACQ ' in text and 'REENTRY result=7 ' not in text:
        print('COAST_ORIGIN delayed_by_reacquisition=1 disable_extrapolation_not_true_stop_speed=1')
    summarize(scaled, coast_scaled, sample_hz, coast_hz, reason)


if __name__ == "__main__":
    main()
