#!/usr/bin/env python3
"""Test the cause of the per-sector ZC wave: LOAD-ANGLE or GEOMETRY?

The closed loop's in-window ZC coverage is one-per-electrical-rev asymmetric -- the first
half of the rev (sectors 0,1,2) crosses late/in-window, the second half (3,4,5) crosses
early/out-of-window. This sweeps AMP at a fixed frequency (closed loop), measures the
per-sector firmware-linfit ZC wave at each amp, and overlays them:

  - if the wave's PHASE slides as amp changes  -> the wave tracks LOAD ANGLE.
  - if it stays nailed to the same sectors     -> fixed motor/sense GEOMETRY.

Amp is the load-angle knob at fixed speed (more torque margin -> different rotor lead).
Each capture's `cl ... lf=` log gives the raw linfit % per sector; we aggregate the
median per physical sector across snaps. Writes logs/wave_<ts>.png + .json. ATTENDED.

Every point is measured INDEPENDENTLY -- no state carried between setpoints, so a stall at
one can't poison the next. Per (hz, amp):
  0. SKIP if below the stall curve (amp < stall_amp(hz)+margin) -- the open-loop rotor slips
     there and the curve predicts it cleanly (verified against manual rotor labels).
  1. q/w/q/w (with pauses) -- aggressive clear of any latched / half-stuck state to idle.
  2. spin up SOLID at a high catch amp (max(amps)+3), then ride amp DOWN to the target --
     descending stays on the hysteretic locked branch; up-ramps near the stall edge drop it.
  3. VERIFY: each capture's debug line must report the target hz AND amp, else it's rejected
     (the ramp didn't land / firmware isn't where we asked).
  4. LOCKED: the linfit must light up >= min_sectors of the 6 sectors -- a synchronously
     spinning rotor produces structured per-sector BEMF; a stalled/slipping one doesn't.
     (Replaces classify_rotor_state, which was unreliable open-loop: false +ve on slip,
     false -ve on lowish duty.)
  5. HOLD: one more capture after the snaps; if it no longer locks, the point didn't hold
     (STALLED-AT-END) and is rejected. Only points that verify, lock, AND hold are recorded;
     everything else is honest nan. Slower (a full re-spin per point) but trustworthy. OPEN
     loop is the clean probe (closed loop re-times the commutation and masks the wave).

  python scripts/cl_wave_sweep.py COM41 --hz 250 --amps 13 15 17 19          # open loop (default)
  python scripts/cl_wave_sweep.py COM41 --freqs 150 250 350 --amps 12 15 18  # the larger (Hz, amp) plane
  python scripts/cl_wave_sweep.py COM41 --freqs 250 300 350 400 --amp-track  # amp window rides up with hz

Each measured setpoint also gets a per-point zc_<hz>_<amp>.png (the same 3-phase ZC render
as latest_zc) under logs/zc_<ts>/ -- so you can eyeball what a 6/6/6 vs 6/5/6 actually looks like.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import statistics as st
import sys
import time
from collections import defaultdict
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

from scope_common import (BAUD, PHASE_NAMES, PHASE_TO_CHANNEL, analyze_zero_crossings,
                          linfit_mb, parse_capture, parse_cl_bounds, plot_zc_snapshot)
from scope_sweep import Setpoint, capture, drain, position, send, set_watchdog, stall_amp


def amps_for(hz, base_amps, track, step, span, margin):
    """The amp list to sweep at `hz`. With --amp-track the window RIDES UP with frequency:
    it starts at the first step-aligned amp at/above the stall edge (stall_amp(hz)+margin) and
    runs `span` points up by `step` -- so every point is above the stall curve by construction
    (no skips) and the higher-freq runs reach correspondingly higher amps. e.g. step 2 span 5
    margin 0.5 gives 250:12-20, 300:14-22, 350:16-24, 400:18-26. Without --amp-track every
    frequency uses the flat --amps list and the curve-skip handles the below-curve points."""
    if not track:
        return list(base_amps)
    start = math.ceil((stall_amp(hz) + margin) / step) * step
    return [start + i * step for i in range(span)]


def _stalled(ser):
    return getattr(ser, "stalled", lambda: False)()


def _clear_stall(ser):
    if hasattr(ser, "clear_stall"):
        ser.clear_stall()


def set_stall_kill(ser, want):
    """Drive the firmware stall-kill to `want` (the 'h' key toggles, so send + read the echo
    and retry if it went the wrong way). Returns the resulting state or None. The coast-run
    stall detector FALSE-FIRES during the open-loop freq-ramp spin-up (the changing frequency
    makes it miscount), killing the motor before it reaches the target -- so sweeps disable
    it and rely on the host verify+lock gate instead."""
    for _ in range(5):
        drain(ser)
        send(ser, "h")
        time.sleep(0.15)
        mo = re.search(r"stall_kill:\s*(on|off)", drain(ser))
        if mo:
            is_on = mo.group(1) == "on"
            if is_on == want:
                return is_on
    return None

# Firmware text-response prefixes -- everything else read is the binary capture payload.
_CTRL_PREFIXES = ("debug:", "reset:", "freq=", "amp=", "trim=", "dump", "end", "cl",
                  "glitch:", "hist:", "regs:", "watchdog", "monitor", "stall", "zc_beta=",
                  "alpha=", "predict_coast=", "kill", "scope_cl2", "STALL")


def _is_control(line: str) -> bool:
    s = line.lstrip()
    return len(s) < 50 or s.startswith(_CTRL_PREFIXES)


def _ts():
    return datetime.now().strftime("%H:%M:%S.%f")[:-3]


class PacedSerial:
    """Transparent Serial wrapper that (1) PACES commands -- a small sleep after every write
    so the firmware has time to finish echoing its response before the next command, else
    back-to-back bytes overrun its 1-byte RX register and the link wedges (the bug that only
    appeared WITHOUT -v, because the print()s were accidentally pacing it); (2) optionally
    ECHOES the conversation to the console (-v/-vv); and (3) always LOGS the TX/RX control
    conversation (timestamped, no binary payload) to `logf` if given, so an unexpected
    stall/choke is easy to pin down after the fact. Everything else proxies through
    __getattr__ so position()/capture()/send() get all three for free."""

    def __init__(self, ser, level=0, write_delay=0.02, logf=None):
        self._ser = ser
        self._level = level
        self._wd = write_delay
        self._logf = logf
        self._rx = ""
        self._stall = False  # set when the firmware reports a STALL kill

    def stalled(self):
        return self._stall

    def clear_stall(self):
        self._stall = False

    def _log(self, s):
        if self._logf:
            self._logf.write(f"{_ts()} {s}\n")
            self._logf.flush()  # keep the log current even if the script then hangs

    def write(self, data):
        s = data.decode("ascii", errors="replace")
        self._log(f"TX> {s!r}")
        if self._level:
            print(f"  TX> {s!r}", flush=True)
        n = self._ser.write(data)
        try:
            self._ser.flush()
        except Exception:
            pass
        if self._wd:
            time.sleep(self._wd)  # let the firmware read+echo before the next command
        return n

    def read(self, n=1):
        data = self._ser.read(n)
        if data:
            self._rx += data.decode("ascii", errors="replace")
            parts = re.split(r"[\r\n]+", self._rx)
            for line in parts[:-1]:
                if not line.strip():
                    continue
                if line.lstrip().startswith("STALL"):
                    self._stall = True  # firmware killed the motor -- caller must react
                is_ctrl = _is_control(line)
                if is_ctrl:
                    self._log(f"RX< {line[:200]}")  # logfile: control lines only, no payload
                # console: -v shows control lines, -vv adds the binary payload, plain is quiet
                if self._level >= 2 or (self._level and is_ctrl):
                    disp = line if len(line) <= 160 else line[:150] + f"...(+{len(line) - 150}B)"
                    print(f"  RX< {disp}", flush=True)
            self._rx = parts[-1]
            if len(self._rx) > 300:  # binary payload streaming without a newline
                if self._level >= 2:
                    print(f"  RX< ...{len(self._rx)}B binary...", flush=True)
                self._rx = self._rx[-40:]
        return data

    def __getattr__(self, name):
        return getattr(self._ser, name)


def set_alpha(ser, alpha: float) -> None:
    send(ser, "0")
    for _ in range(int(round(max(0.0, min(1.0, alpha)) / 0.05))):
        send(ser, "m")
        time.sleep(0.02)


def collect_wave(ser, snaps, timeout, want_hz, want_amp_t, slope_min=15.0, min_sectors=4):
    """Take `snaps` captures at a setpoint; return (median linfit wave, n_locked, n_verified,
    rep_cap) -- rep_cap is the last LOCKED capture (a representative raw frame for rendering a
    per-setpoint zc_<hz>_<amp>.png), or the last verified one if none locked, or None.

    Each capture must pass two gates before it feeds the wave:
      VERIFIED -- the firmware's debug line reports the target hz AND amp (else the ramp didn't
                  land where we asked -> reject).
      LOCKED   -- the linfit lights up >= min_sectors of the 6 sectors. A synchronously
                  spinning rotor produces structured per-sector BEMF in most sectors; a
                  stalled/flat/slipping one lights up few. This REPLACES classify_rotor_state,
                  which was unreliable open-loop (false +ve on slip, false -ve on lowish duty).
    Only verified+locked captures feed the wave; a point that never passes both is honest nan.
    Linfit is ungated by the firmware [-30,130]% (open-loop crossings sit far out-of-window);
    flat windows (|slope| < slope_min) and bonkers extrapolations ([-100,200]%) are dropped."""
    acc: dict[int, list[float]] = defaultdict(list)
    n_locked = n_verified = 0
    rep_cap = None  # last locked capture (preferred) else last verified -- for rendering
    for _ in range(snaps):
        if _stalled(ser):
            break  # firmware killed the motor mid-sequence -- stop capturing a corpse
        try:
            cap = parse_capture(capture(ser, timeout, "c"))
            chz = int(float(cap.debug.get("hz", "x")))
            camp = int(float(cap.debug.get("amp", "x")))  # 0.1% units, e.g. 200 = 20.0%
        except Exception:
            # Malformed / incomplete dump (no dump3 header, truncated, missing debug) --
            # almost always a stall corrupting the capture. Skip the snap, don't crash.
            continue
        if chz != want_hz or abs(camp - want_amp_t) > 2:
            continue  # firmware NOT at the requested setpoint -> not a valid measurement
        n_verified += 1
        if rep_cap is None:
            rep_cap = cap  # at least a verified frame to render if nothing ends up locked
        fps = cap.sample_hz / (chz * 6.0)
        bounds, _ = parse_cl_bounds(cap.text)
        secs, smooth, neutral = analyze_zero_crossings(cap, smooth_window=3, sector_bounds=bounds)
        frames = len(neutral)
        cap_z: dict[int, float] = {}  # this capture's per-sector linfit ZC
        for s in secs:
            ch = PHASE_TO_CHANNEL[PHASE_NAMES.index(s.phase)]
            s0 = int(round(s.start_frame)) + 1
            s1 = min(int(round(s.end_frame)), frames)
            mb = linfit_mb([smooth[ch][f] - neutral[f] for f in range(s0, s1)])
            if mb is None or abs(mb[0]) < slope_min:
                continue  # flat window -> no observable crossing
            z = (-mb[1] / mb[0]) / fps * 100.0
            if -100.0 <= z <= 200.0:
                cap_z[s.index % 6] = z
        if len(cap_z) >= min_sectors:  # enough structured BEMF -> rotor is genuinely spinning
            n_locked += 1
            rep_cap = cap  # prefer a locked frame for the rendered image
            for k, v in cap_z.items():
                acc[k].append(v)
    return {s: st.median(v) for s, v in acc.items() if v}, n_locked, n_verified, rep_cap


def reset_cycles(ser, pause, cycles=2):
    """Thoroughly clear firmware + motor state before a fresh ramp: cycle q (reset) /
    w (kill) with pauses, `cycles` times, so any latched / half-stuck condition settles to a
    known idle before we spin up. Ends killed; the following ramp re-arms via its own 'q'."""
    for _ in range(cycles):
        send(ser, "q")
        time.sleep(pause)
        send(ser, "w")
        time.sleep(pause)
    ser.reset_input_buffer()  # discard the reset/kill echoes


def ramp_amp_down(ser, cur_t, tgt_t, step_pause=0.08):
    """Ramp amp from cur DOWN to tgt (0.1% units) via z (-1%) / - (-0.1%), gently. Descending
    rides the hysteretic locked branch; up-ramps near the stall edge are what drop lock."""
    d = cur_t - tgt_t
    if d <= 0:
        return  # catch should be >= target, so this is a no-op
    for _ in range(d // 10):
        send(ser, "z")
        time.sleep(step_pause)
    for _ in range(d % 10):
        send(ser, "-")
        time.sleep(step_pause)


def measure_point(ser, hz, amp_t, catch_t, alpha, snaps, timeout, settle, reset_pause):
    """Independent measurement of ONE (hz, amp): clear -> catch SOLID at a high amp -> ride
    DOWN to the target -> verify -> measure -> POST-CHECK it still holds. Catch-then-descend
    keeps the rotor on the locked branch; the post-check catches rotors that pass the snaps
    but "stall at the end" (the false-positives the manual-vs-script cross-check exposed).
    Nothing carried between points. Returns (wave, n_locked, n_verified, held, rep_cap)."""
    _clear_stall(ser)
    reset_cycles(ser, reset_pause)  # q,w,q,w -- aggressive clear before ramping
    # Spin up SOLID at the high catch amp (position sends 'q' then ramps freq+amp up).
    position(ser, Setpoint(), hz, catch_t, qsettle=0.8, ramp_step=20,
             ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
    drain(ser)
    if not _stalled(ser):
        ramp_amp_down(ser, catch_t, amp_t)  # ride DOWN to the target (stable branch)
        drain(ser)
    if _stalled(ser):
        # Firmware killed the motor during spin-up/descent -- can't hold lock here. Don't
        # measure a dead motor; the next point's reset_cycles ('q') re-arms it.
        send(ser, "w")
        return {}, 0, 0, None, None
    if alpha > 0:
        set_alpha(ser, alpha)  # 'q' already set open-loop alpha=0
    time.sleep(settle)
    w, nl, nv, rep_cap = collect_wave(ser, snaps, timeout, hz, amp_t)
    # POST-CHECK: one more capture after the snaps. A rotor that "stalls at the end" passes
    # the snaps but is gone now -- if it's still AT the setpoint but no longer locked, the
    # point didn't hold, so reject it (this is the manual-labelled false-positive class).
    held = None
    if nl > 0 and not _stalled(ser):
        _, post_nl, post_nv, _ = collect_wave(ser, 1, timeout, hz, amp_t)
        if post_nv > 0:
            held = post_nl > 0
            if not held:
                return {}, 0, nv, False, rep_cap
    return w, nl, nv, held, rep_cap


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=250, help="single frequency (if --freqs unset)")
    p.add_argument("--freqs", type=int, nargs="+", default=None,
                   help="frequencies to sweep (the ORTHOGONAL load-angle axis); re-spins per freq")
    p.add_argument("--amps", type=float, nargs="+", default=[13, 15, 17, 19],
                   help="amp %% values to sweep (the load-angle knob); flat across freqs "
                        "unless --amp-track")
    p.add_argument("--amp-track", action="store_true",
                   help="ride the amp window UP with frequency instead of a flat --amps list: "
                        "each freq starts at the first --amp-step above the stall edge and runs "
                        "--amp-span points up. e.g. 250:12-20, 300:14-22, 350:16-24, 400:18-26")
    p.add_argument("--amp-span", type=int, default=5,
                   help="number of amp points per frequency when --amp-track (default 5)")
    p.add_argument("--amp-step", type=float, default=2.0,
                   help="amp %% step between points when --amp-track (default 2)")
    p.add_argument("--alpha", type=float, default=0.0,
                   help="0 = open loop (the clean probe; cannot desync). >0 = closed loop")
    p.add_argument("--stall-kill", action="store_true",
                   help="keep the firmware stall-kill ON during the sweep (default OFF -- it "
                        "false-fires during the open-loop freq-ramp spin-up and kills the "
                        "motor before it reaches the target)")
    p.add_argument("--snaps", type=int, default=6)
    p.add_argument("--min-lock", type=int, default=3,
                   help="a point needs at least this many of --snaps captures LOCKED to record")
    p.add_argument("--stall-margin", type=float, default=0.5,
                   help="skip any (hz, amp) below stall_amp(hz)+margin %% -- those slip/stall "
                        "open-loop (the curve predicts it cleanly). 0 disables the skip.")
    p.add_argument("--catch-amp", type=float, default=None,
                   help="amp %% to spin up SOLID at before descending to each target "
                        "(default max(amps)+3); catch high, ride down the locked branch")
    p.add_argument("--settle", type=float, default=1.0)
    p.add_argument("--reset-pause", type=float, default=0.4,
                   help="pause (s) between each q/w in the pre-ramp clear cycle")
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("-v", "--verbose", action="count", default=0,
                   help="-v: log commands (TX) + firmware text responses (RX) live; "
                        "-vv: also dump the full binary capture payload")
    p.add_argument("--cmd-delay", type=float, default=0.02,
                   help="pause (s) after every command so the firmware can read+echo before "
                        "the next -- prevents RX-overrun lockups (0 to disable)")
    p.add_argument("--logfile", type=Path, default=None,
                   help="timestamped TX/RX + status log (default logs/sweep_<ts>.log; "
                        "'' to disable). Control lines only -- no binary payload.")
    p.add_argument("--zc-dir", type=Path, default=None,
                   help="directory for per-setpoint zc_<hz>_<amp>.png renders "
                        "(default logs/zc_<ts>/). '' to disable rendering.")
    args = p.parse_args()

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    freqs = args.freqs or [args.hz]
    # Per-frequency amp list: flat --amps, or (with --amp-track) a window that rides up with hz.
    freq_amps = {hz: amps_for(hz, args.amps, args.amp_track, args.amp_step, args.amp_span,
                              args.stall_margin) for hz in freqs}
    grid: dict[tuple[int, float], dict[int, float]] = {}  # (hz, amp) -> wave

    Path("logs").mkdir(exist_ok=True)
    log_path = args.logfile if args.logfile is not None else Path("logs") / f"sweep_{stamp}.log"
    logf = open(log_path, "w", encoding="utf-8") if str(log_path) else None
    zc_dir = args.zc_dir if args.zc_dir is not None else Path("logs") / f"zc_{stamp}"
    if str(zc_dir):
        zc_dir.mkdir(parents=True, exist_ok=True)

    def render_point(hz, amp, cap):  # one zc_<hz>_<amp>.png like latest_zc, named by setpoint
        if not str(zc_dir) or cap is None:
            return
        out = zc_dir / f"zc_{hz}_{amp:g}.png"
        try:
            plot_zc_snapshot(cap, out, smooth_window=3)
        except Exception as exc:
            both(f"    (render {out.name} skipped: {exc})")

    def both(text):  # print to console AND the timestamped logfile
        print(text)
        if logf:
            logf.write(f"{_ts()} {text}\n")
            logf.flush()

    if logf:
        print(f"logging TX/RX + status to {log_path}")
    with serial.Serial(args.port, args.baud, timeout=0.1) as raw_ser:
        ser = PacedSerial(raw_ser, args.verbose, args.cmd_delay, logf)  # pace + log; verbose layers on
        ser.reset_input_buffer()
        if set_watchdog(ser, True):
            both("watchdog ARMED")
        sk = set_stall_kill(ser, args.stall_kill)
        both(f"firmware stall-kill: {'on' if sk else 'off'}"
             + ("" if args.stall_kill else "  (off -- it false-fires on the open-loop spin-up; "
                "host verify+lock gate guarantees data quality)"))
        both(f"\n  {'hz':>4} {'amp%':>5} | " + " ".join(f"s{s}" for s in range(6))
             + "   ver/lock  status   (every point: w->q->ramp->verify->measure)")
        try:
            for hz in freqs:
                amps = freq_amps[hz]
                # Catch SOLID above the top of this freq's window, then ride down to each target.
                catch_t = int(round((args.catch_amp if args.catch_amp is not None
                                     else max(amps) + 3) * 10))
                for amp in amps:
                    amp_t = int(round(amp * 10))
                    edge = stall_amp(hz) + args.stall_margin
                    if args.stall_margin and amp < edge:
                        # Below the stall curve: the open-loop rotor slips/stalls here (the
                        # curve predicts it cleanly). Don't measure -- it only yields slip
                        # contamination the classifier can't catch. (With --amp-track every
                        # point is above the edge by construction, so this never fires.)
                        both(f"  {hz:>4} {amp:>5.1f} | {' '.join(['  .'] * 6)}   ----  "
                             f"BELOW STALL CURVE (edge ~{edge:.1f}%) -- skipped")
                        continue
                    # FULL independent measurement -- reset, catch high, descend, verify, hold.
                    w, nl, nv, held, rep_cap = measure_point(
                        ser, hz, amp_t, max(catch_t, amp_t), args.alpha, args.snaps,
                        args.capture_timeout, args.settle, args.reset_pause)
                    if _stalled(ser):
                        status = "FW STALL-KILL (motor killed; reset next point)"
                    elif held is False:
                        status = "STALLED-AT-END (passed snaps, lost lock on re-check)"
                    elif nv == 0:
                        status = "NOT-AT-SETPOINT (ramp/echo failed)"
                    elif nl == 0:
                        status = "STALLED (at setpoint, not spinning)"
                    elif nl < args.min_lock:
                        status = f"MARGINAL (lock {nl})"
                    else:
                        status = "ok"
                        grid[(hz, amp)] = w  # only trust a solidly-locked point that HELD
                    row = " ".join(f"{w.get(s, float('nan')):3.0f}" for s in range(6))
                    both(f"  {hz:>4} {amp:>5.1f} | {row}   {nv}/{nl}/{args.snaps}  {status}")
                    render_point(hz, amp, rep_cap)  # zc_<hz>_<amp>.png from a representative frame
        finally:
            send(ser, "w")  # leave the motor killed
            if not args.stall_kill:
                set_stall_kill(ser, True)  # restore the firmware default for other tools
            try:
                set_watchdog(ser, False)
            except Exception:
                pass

    outdir = Path("logs")
    outdir.mkdir(exist_ok=True)
    jpath = outdir / f"wave_{stamp}.json"
    jpath.write_text(json.dumps({f"{hz}/{amp}": w for (hz, amp), w in grid.items()}, indent=2),
                     encoding="ascii")
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        # One panel per frequency: the per-sector wave vs amp (look for slide + flatten).
        n = len(freqs)
        fig, axes = plt.subplots(1, n, figsize=(5 * n, 5), squeeze=False)
        for col, hz in enumerate(freqs):
            ax = axes[0][col]
            for amp in freq_amps[hz]:
                w = grid.get((hz, amp), {})
                ys = [w.get(s, float("nan")) for s in range(6)]
                ax.plot(range(6), ys, "-o", label=f"{amp:.1f}%")
            ax.axhline(50, color="0.7", ls="--", lw=0.8)
            ax.set_title(f"{hz} Hz")
            ax.set_xlabel("physical sector (0..5)")
            ax.set_xticks(range(6))
            ax.grid(alpha=0.3)
            if col == 0:
                ax.set_ylabel("linfit ZC (% of window)")
            ax.legend(title="amp", fontsize=7)
        fig.suptitle("Per-sector ZC wave vs (amp, freq)  "
                     "(slides/flattens with load => load-angle; fixed => geometry)")
        png = outdir / f"wave_{stamp}.png"
        fig.tight_layout()
        fig.savefig(png, dpi=110)
        both(f"\nwave plot -> {png}\njson -> {jpath}")
    except Exception as exc:
        both(f"(plot skipped: {exc}); json -> {jpath}")
    if str(zc_dir):
        both(f"per-setpoint zc renders -> {zc_dir}/zc_<hz>_<amp>.png")
    if logf:
        both(f"log -> {log_path}")
        logf.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
