#!/usr/bin/env python3
"""
pll_track.py — PLL tracking experiments for rinz motor-tester

Modes:
  verify  re-run known-good operating points: pll convergence + active ZC sectors
  step    step between KNOWN_OPS pairs; record pll_est convergence vs time
  ramp    slow freq sweep at fixed amp; log pll_est tracking error vs cmd
  all     verify + step + ramp in sequence (regression suite)

Usage:
  python pll_track.py --mode verify
  python pll_track.py --mode step [--step-pairs 130:180 180:210]
  python pll_track.py --mode ramp [--ramp-start 130] [--ramp-end 270] [--ramp-rate 2]
  python pll_track.py --mode all [--port COM42]

Ctrl-C kills motor and exits cleanly.
"""

import serial
import time
import re
import sys
import signal
import argparse
import csv as csv_mod
from datetime import datetime
from pathlib import Path

# ── defaults ─────────────────────────────────────────────────────────────────
DEFAULT_PORT    = "COM41"
BAUD            = 115200
RESET_FREQ      = 60     # Hz  — 'q' resets here
RESET_AMP_T     = 80     # tenths of %  — 'q' resets here (8.0%)

# Known-good operating points (freq Hz, amp %) from bemf_sweep
KNOWN_OPS = [(130, 9), (180, 14), (210, 14), (270, 13)]

CONV_TOL        = 0.05   # |pll - cmd| / cmd — converged when below this
SETTLE_S        = 15.0   # seconds to wait for initial lock
POLL_S          = 2.0    # seconds between 'i' polls
STEP_DURATION   = 45.0   # seconds to monitor after each step
RAMP_RATE       = 2      # Hz/s
RAMP_AMP        = 12     # % fixed amplitude during ramp
READ_TIMEOUT    = 2.0    # seconds timeout for collecting 'i' response

# ── regex ─────────────────────────────────────────────────────────────────────
TS_RE   = re.compile(r'^\[\s*\d+\.\d\]\s*')
PLL_RE  = re.compile(r'pll_est=([\d.]+)Hz\s+cmd=(\d+)Hz')
ZC_RE   = re.compile(r'zc_hits\s+s0=(\d+)\s+s1=(\d+)\s+s2=(\d+)\s+s3=(\d+)\s+s4=(\d+)\s+s5=(\d+)')
FREQ_RE = re.compile(r'freq=(\d+)Hz')

# ── globals ───────────────────────────────────────────────────────────────────
_ser:    serial.Serial | None = None
_log_fh: object | None        = None


def _log(text: str):
    if _log_fh:
        _log_fh.write(text + '\n')
        _log_fh.flush()


def _kill_and_close():
    if _ser and _ser.is_open:
        try:
            _ser.write(b'w')
            time.sleep(0.15)
        except Exception:
            pass
        _ser.close()
    if _log_fh:
        _log_fh.close()


def _sig(sig, frame):
    print("\n[ABORT] Ctrl-C — killing motor")
    _kill_and_close()
    sys.exit(1)


signal.signal(signal.SIGINT, _sig)


# ── serial helpers ────────────────────────────────────────────────────────────

def _readline(timeout: float = 0.4) -> str:
    _ser.timeout = timeout
    return _ser.readline().decode('ascii', errors='replace').rstrip()


def _drain(t: float = 0.3) -> list[str]:
    deadline = time.time() + t
    lines = []
    while time.time() < deadline:
        _ser.timeout = 0.05
        l = _ser.readline().decode('ascii', errors='replace').rstrip()
        if l:
            lines.append(l)
    return lines


def _strip_ts(line: str) -> str:
    return TS_RE.sub('', line)


# ── motor control ─────────────────────────────────────────────────────────────

def _set_freq(target: int, current: int,
              step_hz: int = 5, step_delay: float = 0.4) -> int:
    """Ramp with per-step freq readback — reliable, for pre-positioning."""
    key = b'd' if target > current else b'c'
    steps = 0
    while current != target:
        _ser.write(key)
        line = _readline(0.3)
        if not (m := FREQ_RE.search(line)):
            deadline = time.time() + 1.0
            while time.time() < deadline:
                line = _readline(0.15)
                if m := FREQ_RE.search(line):
                    break
        if m:
            current = int(m.group(1))
        steps += 1
        if steps >= step_hz and current != target:
            time.sleep(step_delay)
            steps = 0
    return current


def _burst_freq(delta: int):
    """Send |delta| d/c key presses at 10 ms intervals without readback (fast step)."""
    if delta == 0:
        return
    key = b'd' if delta > 0 else b'c'
    for _ in range(abs(delta)):
        _ser.write(key)
        time.sleep(0.01)
    _drain(0.3)


def _set_amp(target_t: int, current_t: int) -> int:
    """Set amplitude in tenths of % (floor 70, ceiling 200). Uses a/z + +/- keys."""
    target_t = max(70, min(target_t, 200))
    delta = target_t - current_t
    if delta == 0:
        return current_t
    coarse, fine = abs(delta) // 10, abs(delta) % 10
    ck = b'a' if delta > 0 else b'z'
    fk = b'+' if delta > 0 else b'-'
    for _ in range(coarse):
        _ser.write(ck)
        time.sleep(0.05)
    for _ in range(fine):
        _ser.write(fk)
        time.sleep(0.05)
    _drain(0.15)
    return target_t


def _reset() -> tuple[int, int]:
    """Send 'q'. Returns (RESET_FREQ, RESET_AMP_T)."""
    _ser.write(b'q')
    time.sleep(0.5)
    _drain(0.5)
    return RESET_FREQ, RESET_AMP_T


def _goto(freq: int, amp_pct: int, cur_f: int, cur_a: int) -> tuple[int, int]:
    """Ramp to (freq Hz, amp_pct %). Returns (new_freq, new_amp_tenths)."""
    cur_a = _set_amp(amp_pct * 10, cur_a)
    time.sleep(0.2)
    cur_f = _set_freq(freq, cur_f, step_hz=5, step_delay=0.4)
    time.sleep(0.3)
    return cur_f, cur_a


# ── 'i' status reader ─────────────────────────────────────────────────────────

def _read_i(timeout: float = READ_TIMEOUT) -> tuple[float, int, list[int]] | None:
    """Send 'i'. Returns (pll_est, cmd_freq, zc_hits[6]) or None."""
    _ser.write(b'i')
    pll_est = cmd_freq = None
    zc = None
    deadline = time.time() + timeout
    while time.time() < deadline:
        line = _readline(0.3)
        if not line:
            continue
        _log(line)
        s = _strip_ts(line)
        if m := PLL_RE.search(s):
            pll_est, cmd_freq = float(m.group(1)), int(m.group(2))
        if m := ZC_RE.search(s):
            zc = [int(m.group(i + 1)) for i in range(6)]
        if pll_est is not None and zc is not None:
            break
    if pll_est is not None and cmd_freq is not None and zc is not None:
        return (pll_est, cmd_freq, zc)
    return None


def _active(zc: list[int]) -> int:
    return sum(1 for x in zc if x > 0)


def _fmt_zc(zc: list[int]) -> str:
    return '[' + ','.join(f'{x:5d}' for x in zc) + ']'


# ── mode: verify ──────────────────────────────────────────────────────────────

def mode_verify(args, cw) -> bool:
    """Check each known operating point: pll convergence + active ZC sectors."""
    print(f"\n{'═'*62}")
    print("  VERIFY — known operating points")
    print(f"{'═'*62}")
    _log("\n=== VERIFY ===")

    all_pass = True
    cur_f, cur_a = _reset()

    for freq, amp in KNOWN_OPS:
        _log(f"\n--- {freq}Hz/{amp}% ---")
        print(f"\n  {freq}Hz/{amp}%: positioning ...", flush=True)
        cur_f, cur_a = _goto(freq, amp, cur_f, cur_a)
        print(f"  {freq}Hz/{amp}%: waiting for lock (up to {args.settle:.0f}s) ...", flush=True)

        t0 = time.time()
        converged = False
        pll_last = cmd_last = None
        zc_last = None

        while time.time() - t0 < args.settle:
            r = _read_i()
            if r:
                pll_last, cmd_last, zc_last = r
                if abs(pll_last - freq) / freq < args.conv_tol:
                    converged = True
                    break
            time.sleep(args.poll)

        if pll_last is None:
            print(f"  {freq}Hz/{amp}%: no i response — FAIL")
            all_pass = False
            if cw:
                cw.writerow(['verify', freq, amp, 'nan', freq, 'nan', 0, *[0]*6])
            continue

        err = (pll_last - freq) / freq * 100.0
        act = _active(zc_last)
        passed = converged and act >= 1
        verdict = 'PASS ✓' if passed else 'FAIL ✗'
        if not passed:
            all_pass = False

        elapsed = time.time() - t0
        conv_info = f"converged at {elapsed:.0f}s" if converged else f"not converged after {args.settle:.0f}s"
        print(f"  {freq}Hz/{amp}%  pll={pll_last:.1f}Hz  err={err:+.1f}%  {act} sectors  {verdict}  ({conv_info})")
        print(f"    zc_hits={_fmt_zc(zc_last)}")

        if cw:
            cw.writerow(['verify', freq, amp,
                         f'{pll_last:.2f}', freq, f'{err:.2f}', act, *zc_last])

    return all_pass


# ── mode: step ────────────────────────────────────────────────────────────────

def mode_step(args, cw) -> bool:
    """Step between consecutive KNOWN_OPS pairs; record pll_est convergence vs time."""
    if args.step_pairs:
        pairs = []
        for spec in args.step_pairs:
            fa, fb = map(int, spec.split(':'))
            try:
                opa = next(op for op in KNOWN_OPS if op[0] == fa)
                opb = next(op for op in KNOWN_OPS if op[0] == fb)
            except StopIteration:
                print(f"ERROR: freq in '{spec}' not in KNOWN_OPS {[f for f, _ in KNOWN_OPS]}")
                sys.exit(1)
            pairs.append((opa, opb))
    else:
        pairs = [(KNOWN_OPS[i], KNOWN_OPS[i + 1]) for i in range(len(KNOWN_OPS) - 1)]

    all_pass = True

    for (fa, aa), (fb, ab) in pairs:
        print(f"\n{'═'*62}")
        print(f"  STEP: {fa}Hz/{aa}% → {fb}Hz/{ab}%")
        print(f"{'═'*62}")
        _log(f"\n=== STEP {fa}Hz/{aa}% → {fb}Hz/{ab}% ===")

        cur_f, cur_a = _reset()
        print(f"  Going to {fa}Hz/{aa}% ...", flush=True)
        cur_f, cur_a = _goto(fa, aa, cur_f, cur_a)
        print(f"  Settling {args.settle:.0f}s at {fa}Hz ...", flush=True)

        t0 = time.time()
        locked = False
        while time.time() - t0 < args.settle:
            r = _read_i()
            if r and abs(r[0] - fa) / fa < args.conv_tol:
                locked = True
                break
            time.sleep(args.poll)
        print(f"  Pre-step state: {'locked ✓' if locked else 'not converged (continuing)'}")

        # ZC snapshot before step — for delta display after
        r_pre = _read_i()
        zc_base = r_pre[2] if r_pre else [0] * 6

        # Fast burst to B
        print(f"  Bursting {fa}→{fb}Hz + amp {aa}→{ab}% ...", flush=True)
        cur_a = _set_amp(ab * 10, cur_a)
        _burst_freq(fb - fa)
        t_step = time.time()
        print(f"  [t=0]")

        print(f"\n  {'t(s)':>6}  {'pll_est':>8}  {'cmd':>5}  {'err%':>6}  act  zc_delta")
        print(f"  {'─'*68}")

        step_pass = False
        conv_t = None

        while time.time() - t_step < args.step_duration:
            time.sleep(args.poll)
            r = _read_i()
            if r is None:
                continue
            pll, cmd, zc = r
            t = time.time() - t_step
            err = (pll - fb) / fb * 100.0
            act = _active(zc)
            zc_d = [zc[i] - zc_base[i] for i in range(6)]
            conv = abs(pll - fb) / fb < args.conv_tol
            if conv and not step_pass:
                step_pass = True
                conv_t = t
            marker = ' ✓' if conv else ''
            print(f"  {t:>6.1f}s  {pll:>7.1f}Hz  {fb:>5}Hz  {err:>+6.1f}%  {act:>3}  {_fmt_zc(zc_d)}{marker}")
            if cw:
                cw.writerow(['step', fa, fb, f'{t:.1f}', f'{pll:.2f}',
                             fb, f'{err:.2f}', act, *zc])

        if step_pass:
            print(f"\n  → PASS ✓  converged at t={conv_t:.1f}s")
        else:
            r = _read_i()
            last = f'{r[0]:.1f}Hz' if r else '?'
            print(f"\n  → FAIL ✗  pll_est={last} after {args.step_duration:.0f}s (target {fb}Hz, tol {args.conv_tol*100:.0f}%)")
            all_pass = False

    return all_pass


# ── mode: ramp ────────────────────────────────────────────────────────────────

def mode_ramp(args, cw) -> bool:
    """Slow freq ramp at fixed amp; log pll_est tracking error vs cmd freq.

    Actual ramp rate at poll steps will be slower than nominal due to 'i' response
    time. At rate=2Hz/s poll_every=5, expect ~1.7Hz/s effective rate.
    """
    start_f    = args.ramp_start
    end_f      = args.ramp_end
    rate       = args.ramp_rate
    amp        = args.ramp_amp
    poll_every = args.ramp_poll_every
    step_delay = 1.0 / rate

    print(f"\n{'═'*62}")
    print(f"  RAMP: {start_f}Hz → {end_f}Hz @ {rate}Hz/s  amp={amp}%  poll every {poll_every}Hz")
    print(f"{'═'*62}")
    _log(f"\n=== RAMP {start_f}→{end_f}Hz @ {rate}Hz/s amp={amp}% ===")

    cur_f, cur_a = _reset()
    print(f"  Going to {start_f}Hz/{amp}% ...", flush=True)
    cur_f, cur_a = _goto(start_f, amp, cur_f, cur_a)
    print(f"  Settling {args.settle:.0f}s ...", flush=True)

    t0 = time.time()
    while time.time() - t0 < args.settle:
        r = _read_i()
        if r and abs(r[0] - start_f) / start_f < args.conv_tol:
            break
        time.sleep(args.poll)
    print("  ok")

    direction = 1 if end_f >= start_f else -1
    key = b'd' if direction > 0 else b'c'
    cmd_f = start_f
    max_err = 0.0
    n_pts = 0

    print(f"\n  {'cmd':>6}  {'pll_est':>8}  {'err%':>6}  act  zc_hits")
    print(f"  {'─'*60}")

    while cmd_f != end_f:
        t_step_start = time.time()
        _ser.write(key)
        cmd_f += direction

        should_poll = (abs(cmd_f - start_f) % poll_every == 0) or cmd_f == end_f
        if should_poll:
            # Use shorter timeout to stay close to ramp rate
            poll_to = min(READ_TIMEOUT, max(0.5, step_delay * poll_every * 0.8))
            r = _read_i(timeout=poll_to)
            if r:
                pll, _, zc = r
                err = (pll - cmd_f) / cmd_f * 100.0
                act = _active(zc)
                max_err = max(max_err, abs(err))
                n_pts += 1
                print(f"  {cmd_f:>6}Hz  {pll:>7.1f}Hz  {err:>+6.1f}%  {act:>3}  {_fmt_zc(zc)}")
                if cw:
                    cw.writerow(['ramp', cmd_f, amp, f'{pll:.2f}',
                                 cmd_f, f'{err:.2f}', act, *zc])

        elapsed = time.time() - t_step_start
        remaining = step_delay - elapsed
        if remaining > 0.005:
            time.sleep(remaining)

    ramp_pass = max_err < 15.0
    print(f"\n  → RAMP: {'PASS ✓' if ramp_pass else 'FAIL ✗'}  peak err={max_err:.1f}%  ({n_pts} samples)")
    return ramp_pass


# ── mode: all ─────────────────────────────────────────────────────────────────

def mode_all(args, cw) -> bool:
    """Run verify + step + ramp in sequence; print combined pass/fail."""
    results = {
        'verify': mode_verify(args, cw),
        'step':   mode_step(args, cw),
        'ramp':   mode_ramp(args, cw),
    }

    print(f"\n{'═'*62}")
    print("  REGRESSION SUMMARY")
    print(f"{'═'*62}")
    overall = True
    for name, passed in results.items():
        print(f"  {name:<10}  {'PASS ✓' if passed else 'FAIL ✗'}")
        overall = overall and passed
    print(f"  {'─'*20}")
    print(f"  {'OVERALL':<10}  {'PASS ✓' if overall else 'FAIL ✗'}")
    return overall


# ── main ──────────────────────────────────────────────────────────────────────

def main():
    global _ser, _log_fh

    ap = argparse.ArgumentParser(
        description='PLL tracking experiments for rinz motor-tester',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
modes:
  verify  re-run known operating points; check pll convergence + active ZC sectors
  step    step between KNOWN_OPS pairs; record convergence time
  ramp    slow freq sweep at fixed amp; log tracking error
  all     verify + step + ramp (regression suite)
""")
    ap.add_argument('--mode', choices=['step', 'ramp', 'verify', 'all'],
                    default='verify', metavar='MODE',
                    help='Test mode: verify|step|ramp|all (default: verify)')
    ap.add_argument('--port', default=DEFAULT_PORT)

    # timing / convergence
    ap.add_argument('--settle',        type=float, default=SETTLE_S,      metavar='s',
                    dest='settle',
                    help=f'Seconds to wait for initial PLL lock (default {SETTLE_S})')
    ap.add_argument('--poll',          type=float, default=POLL_S,        metavar='s',
                    help=f'Seconds between i polls (default {POLL_S})')
    ap.add_argument('--step-duration', type=float, default=STEP_DURATION, metavar='s',
                    dest='step_duration',
                    help=f'Seconds to monitor after each step (default {STEP_DURATION})')
    ap.add_argument('--conv-tol',      type=float, default=CONV_TOL,      metavar='frac',
                    dest='conv_tol',
                    help=f'Convergence: |err|/cmd < tol (default {CONV_TOL} = 5%%)')

    # step mode
    ap.add_argument('--step-pairs', nargs='+', metavar='Hz:Hz', dest='step_pairs',
                    help='Override step pairs, e.g. --step-pairs 130:180 210:270')

    # ramp mode
    ap.add_argument('--ramp-start',      type=int,   default=130,       dest='ramp_start')
    ap.add_argument('--ramp-end',        type=int,   default=270,       dest='ramp_end')
    ap.add_argument('--ramp-rate',       type=float, default=RAMP_RATE, dest='ramp_rate',
                    metavar='Hz/s',
                    help=f'Freq ramp rate in Hz/s (default {RAMP_RATE})')
    ap.add_argument('--ramp-amp',        type=int,   default=RAMP_AMP,  dest='ramp_amp',
                    metavar='%',
                    help=f'Fixed amplitude %% during ramp (default {RAMP_AMP})')
    ap.add_argument('--ramp-poll-every', type=int,   default=5,         dest='ramp_poll_every',
                    metavar='Hz',
                    help='Poll i every N Hz steps during ramp (default 5)')

    # output
    ap.add_argument('--log-dir', default='logs', metavar='DIR',
                    help='Directory for log + CSV files (default: logs/)')

    args = ap.parse_args()

    # open log + CSV
    log_dir = Path(args.log_dir)
    log_dir.mkdir(parents=True, exist_ok=True)
    tag = datetime.now().strftime('%Y%m%d_%H%M%S')
    log_path = log_dir / f"pll_track_{args.mode}_{tag}.txt"
    csv_path = log_dir / f"pll_track_{args.mode}_{tag}.csv"

    _log_fh = open(log_path, 'w', encoding='utf-8')
    _log(f"# pll_track  mode={args.mode}  {datetime.now().isoformat(timespec='seconds')}")
    _log(f"# {' '.join(sys.argv)}")
    _log('')
    print(f"Log → {log_path}")
    print(f"CSV → {csv_path}")

    csv_fh = open(csv_path, 'w', newline='', encoding='utf-8')
    cw = csv_mod.writer(csv_fh)
    cw.writerow(['mode', 'fa_or_cmd', 'amp', 'pll_est', 'cmd',
                 'err_pct', 'active_sectors', 'zc0', 'zc1', 'zc2', 'zc3', 'zc4', 'zc5'])

    print(f"Opening {args.port} @ {BAUD} ...")
    _ser = serial.Serial(args.port, BAUD, timeout=1.0)
    time.sleep(0.3)
    _drain()

    try:
        if args.mode == 'verify':
            mode_verify(args, cw)
        elif args.mode == 'step':
            mode_step(args, cw)
        elif args.mode == 'ramp':
            mode_ramp(args, cw)
        else:
            mode_all(args, cw)
    finally:
        try:
            _ser.write(b'w')
            time.sleep(0.1)
        except Exception:
            pass
        csv_fh.close()
        _kill_and_close()
        print(f"\nDone. Log: {log_path}  CSV: {csv_path}")


if __name__ == '__main__':
    main()
