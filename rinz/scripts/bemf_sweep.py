#!/usr/bin/env python3
"""
bemf_sweep.py — sweep amplitude at fixed RPM points to find centered BEMF ZC

Sends 'e' command N times per amp step, scores each point by:
  - number of in-window sectors (primary)
  - centeredness of ZC index within the sector window (secondary)

Usage:
  python bemf_sweep.py [--port COM41] [--amp-max 20] [--freqs 130 210] [--repeats 3]
                       [--freq-limits 130:13 210:17 270:18]

--freq-limits overrides --amp-max for specific frequencies.
Example: low frequencies need a lower ceiling to avoid overcurrent;
  high frequencies can go higher but stall above ~420 Hz at amp=20%.

NOTE: close any other serial terminal (ss, etc.) on COM41 before running.
Ctrl-C sends 'w' (motor kill) before exiting.
"""

import serial
import time
import re
import sys
import signal
import argparse
from datetime import datetime
from pathlib import Path

# ── defaults ────────────────────────────────────────────────────────────────
DEFAULT_PORT   = "COM41"
BAUD           = 115200
RESET_FREQ     = 60       # Hz — 'q' always resets here
RESET_AMP      = 8        # % — 'q' always resets here
DEFAULT_FREQS  = [130, 210]
AMP_MIN        = 7        # % — ADC timing floor
DEFAULT_AMP_MAX = 20      # % — user-confirmed safe ceiling
DEFAULT_REPEATS = 3       # 'e' calls per amp step
SETTLE_S        = 0.35    # seconds after amp change before sampling
READ_TIMEOUT    = 4.0     # seconds to wait for full 'e' dump

ZC_RE = re.compile(r'\[s(\d) [^\]]*?(ZC@(\d+)/(\d+)|ZC<0|ZC>(\d+))\]')

# ── globals ──────────────────────────────────────────────────────────────────
_ser:     serial.Serial | None = None
_log_fh:  object | None        = None   # open file handle for raw dump log


def _log(text: str):
    if _log_fh:
        _log_fh.write(text + '\n')
        _log_fh.flush()


def _kill_and_close():
    if _ser and _ser.is_open:
        _ser.write(b'w')
        time.sleep(0.15)
        _ser.close()
    if _log_fh:
        _log_fh.close()


def _sig(sig, frame):
    print("\n[ABORT] Ctrl-C — killing motor")
    _kill_and_close()
    sys.exit(1)


signal.signal(signal.SIGINT, _sig)


# ── low-level serial helpers ─────────────────────────────────────────────────

def _readline(timeout=0.4) -> str:
    _ser.timeout = timeout
    return _ser.readline().decode('ascii', errors='replace').rstrip()


def _drain(t=0.3) -> list[str]:
    deadline = time.time() + t
    lines = []
    while time.time() < deadline:
        _ser.timeout = 0.05
        l = _ser.readline().decode('ascii', errors='replace').rstrip()
        if l:
            lines.append(l)
    return lines


# ── motor control ────────────────────────────────────────────────────────────

def _set_freq(target: int, current: int, step_hz: int = 5, step_delay: float = 0.5) -> int:
    """Ramp freq to target in step_hz increments with step_delay between each.

    Uses d/c (±1 Hz) exclusively so step size is fully controllable.
    """
    key = b'd' if target > current else b'c'
    steps_since_delay = 0

    while current != target:
        _ser.write(key)
        # read back freq confirmation
        line = _readline(0.3)
        if not (m := re.search(r'freq=(\d+)Hz', line)):
            deadline = time.time() + 1.0
            while time.time() < deadline:
                line = _readline(0.15)
                if m := re.search(r'freq=(\d+)Hz', line):
                    break
        if m:
            current = int(m.group(1))

        steps_since_delay += 1
        if steps_since_delay >= step_hz and current != target:
            time.sleep(step_delay)
            steps_since_delay = 0

    return current


def _set_amp(target_tenths: int, current_tenths: int, amp_max_tenths: int) -> int:
    """Step amp to target in tenths-of-percent, respecting ceiling.

    Uses 'a'/'z' for whole-percent steps (10 tenths) and '+'/'-' for
    fine 0.1% steps (1 tenth) to minimise keystrokes.
    All values in tenths (110 = 11.0%).
    """
    target_tenths = min(target_tenths, amp_max_tenths)
    delta = target_tenths - current_tenths
    if delta == 0:
        return current_tenths

    coarse = abs(delta) // 10
    fine   = abs(delta) %  10
    coarse_key = b'a' if delta > 0 else b'z'
    fine_key   = b'+' if delta > 0 else b'-'

    for _ in range(coarse):
        _ser.write(coarse_key)
        time.sleep(0.05)
    for _ in range(fine):
        _ser.write(fine_key)
        time.sleep(0.05)
    _drain(0.15)
    return target_tenths


# ── 'e' dump parser ──────────────────────────────────────────────────────────

def _read_e_dump(log_header: str = '') -> list[tuple] | None:
    """
    Send 'e', collect and parse all 6 sector lines.
    Returns list of (sector, in_window, zc_idx|None, zc_total|None),
    or None if motor stopped or mode mismatch.
    All received lines are written verbatim to the log file.
    """
    if log_header:
        _log(f'\n=== {log_header} ===')
    _ser.write(b'e')
    results = []
    deadline = time.time() + READ_TIMEOUT
    while time.time() < deadline:
        line = _readline(0.5)
        if not line:
            continue
        _log(line)
        if 'motor not running' in line or 'only in triggered' in line:
            return None
        m = ZC_RE.search(line)
        if m:
            sec  = int(m.group(1))
            zcs  = m.group(2)
            if zcs.startswith('ZC@'):
                results.append((sec, True, int(m.group(3)), int(m.group(4))))
            else:
                results.append((sec, False, None, None))
        if len(results) == 6:
            break
    return results if len(results) == 6 else None


# ── scoring ──────────────────────────────────────────────────────────────────

def _score_runs(runs: list[list[tuple]]) -> tuple[float, float, dict, dict]:
    """
    Returns:
      avg_in_window   — average in-window sector count per revolution
      avg_centered    — average centeredness (0..1) of in-window crossings
      zc_by_sector    — {sector: [(idx, total), ...]} for all in-window hits
      hits_by_sector  — {sector: hit_count} — how many runs had that sector in-window
    """
    total_iw  = 0
    total_ctr = 0.0
    zc_by_sec:   dict[int, list] = {}
    hits_by_sec: dict[int, int]  = {}

    for run in runs:
        for sec, iw, idx, total in run:
            if iw:
                total_iw  += 1
                ctr = 1.0 - abs(idx - total / 2) / (total / 2)
                total_ctr += ctr
                zc_by_sec.setdefault(sec, []).append((idx, total))
                hits_by_sec[sec] = hits_by_sec.get(sec, 0) + 1

    n_runs = len(runs)
    avg_iw  = total_iw  / n_runs
    avg_ctr = total_ctr / total_iw if total_iw else 0.0
    return avg_iw, avg_ctr, zc_by_sec, hits_by_sec


# ── main ─────────────────────────────────────────────────────────────────────

def main():
    global _ser

    ap = argparse.ArgumentParser(description='BEMF ZC amplitude sweep for rinz motor-tester')
    ap.add_argument('--port',        default=DEFAULT_PORT)
    ap.add_argument('--amp-min',     type=int, default=AMP_MIN,          metavar='%')
    ap.add_argument('--amp-max',     type=int, default=DEFAULT_AMP_MAX,  metavar='%')
    ap.add_argument('--freqs',       type=int, nargs='+', default=DEFAULT_FREQS, metavar='Hz')
    ap.add_argument('--repeats',     type=int,   default=DEFAULT_REPEATS)
    ap.add_argument('--ramp-step',   type=int,   default=5,   metavar='Hz',
                    help='Hz per ramp increment (default 5)')
    ap.add_argument('--ramp-delay',  type=float, default=0.5, metavar='sec',
                    help='Seconds to wait after each ramp increment (default 0.5)')
    ap.add_argument('--freq-limits', nargs='+', default=[], metavar='Hz:MIN%:MAX%',
                    help='Per-frequency amp bounds, e.g. 130:7:13 210:11:17 270:13:19'
                         ' (MIN optional: 130:13 sets only ceiling)')
    ap.add_argument('--log-dir',     default='logs', metavar='DIR',
                    help='Directory for raw dump log (default: logs/)')
    args = ap.parse_args()

    # Build per-frequency (min, max) amp bounds table.
    # All user-facing values are in whole percent; internally we use tenths.
    def to_tenths(pct: int) -> int:
        return pct * 10

    freq_bounds: dict[int, tuple[int, int]] = {}
    for spec in args.freq_limits:
        parts = spec.split(':')
        try:
            if len(parts) == 2:
                freq_bounds[int(parts[0])] = (to_tenths(args.amp_min), to_tenths(int(parts[1])))
            elif len(parts) == 3:
                freq_bounds[int(parts[0])] = (to_tenths(int(parts[1])), to_tenths(int(parts[2])))
            else:
                raise ValueError
        except ValueError:
            print(f"ERROR: bad --freq-limits entry '{spec}', expected Hz:MAX% or Hz:MIN%:MAX%")
            sys.exit(1)

    def amp_floor(freq: int) -> int:
        return freq_bounds[freq][0] if freq in freq_bounds else to_tenths(args.amp_min)

    def amp_ceiling(freq: int) -> int:
        return freq_bounds[freq][1] if freq in freq_bounds else to_tenths(args.amp_max)

    def fmt_amp(tenths: int) -> str:
        return f"{tenths // 10}.{tenths % 10}"

    if args.amp_max > DEFAULT_AMP_MAX:
        print(f"WARNING: --amp-max {args.amp_max}% exceeds validated ceiling {DEFAULT_AMP_MAX}%. Proceeding.")

    # Open log file
    global _log_fh
    log_dir = Path(args.log_dir)
    log_dir.mkdir(parents=True, exist_ok=True)
    log_path = log_dir / f"sweep_{datetime.now().strftime('%Y%m%d_%H%M%S')}.txt"
    _log_fh = open(log_path, 'w', encoding='utf-8')
    _log(f"# bemf_sweep  {datetime.now().isoformat(timespec='seconds')}")
    _log(f"# {' '.join(sys.argv)}")
    _log('')
    print(f"Logging raw dumps → {log_path}")

    print(f"Opening {args.port} @ {BAUD} ...")
    _ser = serial.Serial(args.port, BAUD, timeout=1.0)
    time.sleep(0.3)
    _drain()

    # Reset to known state
    print("Resetting motor (q) ...")
    _ser.write(b'q')
    time.sleep(0.5)
    _drain(0.4)

    current_freq = RESET_FREQ
    current_amp  = RESET_AMP
    summary: list[tuple] = []

    for freq in args.freqs:
        floor = amp_floor(freq)
        ceil  = amp_ceiling(freq)
        print(f"\n{'═'*56}")
        print(f"  {freq} Hz   amp sweep {fmt_amp(ceil)}% ↓ {fmt_amp(floor)}%   ({args.repeats} samples/step)")
        print(f"{'═'*56}")

        # Set amp to this freq's ceiling BEFORE ramping — keeps motor synced during transition
        print(f"  amp → {fmt_amp(ceil)}%", end=' ... ', flush=True)
        current_amp = _set_amp(ceil, current_amp, ceil)
        time.sleep(SETTLE_S)
        print("ok")

        # Ramp to target frequency
        print(f"  freq {current_freq} → {freq} Hz", end=' ... ', flush=True)
        current_freq = _set_freq(freq, current_freq, step_hz=args.ramp_step, step_delay=args.ramp_delay)
        time.sleep(SETTLE_S)
        print(f"ok")

        n_runs = args.repeats
        freq_results = []
        print(f"  {'amp':>5}  {'in-win':>6}  {'ctr':>5}  {'consist':>8}  details")
        print(f"  {'─'*60}")

        # Sweep amp DOWN from ceiling to floor — starting from stable high-drive state
        for amp in range(ceil, floor - 1, -1):
            if amp < ceil:
                current_amp = _set_amp(amp, current_amp, ceil)
                time.sleep(SETTLE_S)

            runs = []
            for rep in range(n_runs):
                header = f"freq={freq}Hz amp={fmt_amp(amp)}% run={rep+1}/{n_runs}"
                result = _read_e_dump(log_header=header)
                if result is None:
                    print(f"\n  amp={fmt_amp(amp)}%: motor stopped — aborting sweep")
                    _kill_and_close()
                    sys.exit(1)
                runs.append(result)
                time.sleep(0.08)

            avg_iw, avg_ctr, zc_by_sec, hits_by_sec = _score_runs(runs)

            # Consistency: "sX:N/M" per sector
            consist_parts = [f"s{s}:{hits_by_sec[s]}/{n_runs}" for s in sorted(hits_by_sec)]
            consist_str = ' '.join(consist_parts) if consist_parts else '—'

            # Detail: ZC index spread per sector
            parts = []
            for sec in sorted(zc_by_sec):
                idxs = '+'.join(f"{i}/{t}" for i, t in zc_by_sec[sec])
                parts.append(f"s{sec}:{idxs}")
            detail = '  '.join(parts) if parts else 'none'

            marker = ' ◄' if avg_iw >= 1.0 else ''
            print(f"  {fmt_amp(amp):>5}%  {avg_iw:>6.1f}  {avg_ctr:>5.2f}  {consist_str:<8}  {detail}{marker}")

            freq_results.append((amp, avg_iw, avg_ctr, zc_by_sec, hits_by_sec))

        # Best: most in-window sectors, tie-break by centeredness
        best = max(freq_results, key=lambda r: (r[1], r[2]))
        summary.append((freq, best))
        # current_amp is now at floor, ready for next freq's pre-ramp set

    # Kill motor
    _ser.write(b'w')
    time.sleep(0.1)

    print(f"\n{'═'*56}")
    print("  BEST OPERATING POINTS")
    print(f"{'═'*56}")
    print(f"  {'freq':>6}  {'amp':>5}  {'in-win':>6}  {'ctr':>5}  consistency  sectors")
    for freq, (amp, iw, ctr, zc_by_sec, hits_by_sec) in summary:
        n = args.repeats
        consist_parts = [f"s{s}:{hits_by_sec[s]}/{n}" for s in sorted(hits_by_sec)]
        consist_str = ' '.join(consist_parts) if consist_parts else '—'
        secs = ', '.join(f"s{s}" for s in sorted(zc_by_sec)) if zc_by_sec else 'none'
        print(f"  {freq:>6}  {fmt_amp(amp):>5}%  {iw:>6.1f}  {ctr:>5.2f}  {consist_str:<12}  {secs}")

    _ser.close()
    if _log_fh:
        _log_fh.close()
    print(f"\nDone. Raw log: {log_path}")


if __name__ == '__main__':
    main()
