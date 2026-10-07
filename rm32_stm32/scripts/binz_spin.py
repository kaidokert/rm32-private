#!/usr/bin/env python3
"""binz bring-up spin: arm rm32, hold a LOW duty for a few seconds, stop, and
report the feedback channels (bus mV, current, eHz, nFAULT) from the
firmware's `i` + `b` lines.

    python scripts/binz_spin.py --arm-only                 # stage 1: no drive
    python scripts/binz_spin.py --duty 10 --hold 10        # stage 2

Safety (bench rules, non-negotiable):
- duty is capped at 15 %; anything higher is refused.
- 'w' (kill: all off, throttle 0) is sent on EVERY exit path — normal end,
  abort, exception, Ctrl-C.
- the firmware's own guard (500 mA, >10 % sag, 9 V floor, nFAULT) latches
  independently; this script aborts on killed=1, nf=1, a stalled/never-run
  motor, or a host-side >10 % bus sag.
- queries ('i') are rate-limited while driving: UART TX couples into the
  comparator on this bench.

Everything received is written to --out (default captures/binz/spin_<ts>.txt).
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import time

import serial

PORT = "COM41"
BAUD = 115_200
MAX_DUTY = 15
# Envelope climb cap, APPLIED duty in tenths of a percent (`--rung`). Raised
# one rung at a time, only after that rung's gates pass (binz climb goal).
# 2026-10-02 climb (1 A): 32.5 %. 2026-10-03 climb (2.5 A): 57.5 % clean 3/3;
# 60.0 % holds 3/3 at 2.03 A (stop current) with one post-hold SAG trip.
# 2026-10-03 climb (5 A): 77.5 % 3/3 at 3.92 A (stop current).
MAX_RUNG_TENTHS = 1000


def rung_command(tenths: int) -> int:
    """Bench permille command whose sine-start-mapped duty is `tenths`.

    Inverts the firmware chain (sine start, changeover 5 %, minimum 40):
      duty  = 80 + (m - 137) * 1920 / 1910       (compute_setpoint, sine map)
      m     = 160 + (a - 100) * 1887 / 1947      (sine_start_map, a >= 100)
      a     = 47 + 2 * permille                  (bench_input permille)
    Checked: bench 13 % (a = 307) -> duty 304 measured.
    """
    d = tenths * 2  # per-2000
    m = 137 + (d - 80) * 1910 / 1920
    a = 100 + (m - 160) * 1947 / 1887
    p = round((a - 47) / 2)
    if not 101 <= p <= 1000:
        raise SystemExit(f"rung {tenths} -> permille {p} outside 101..1000")
    return p


def walk_commands(target: str, step_pm: int = 10) -> list[str]:
    """Changeover (5 %) then +step_pm permille per step (1 s each) up to,
    not including, the target. Above ~30 % duty a 1 % step's acceleration
    current alone exceeds ~0.4 A on this rotor (binz climb), so the climb
    walks in 0.5 % steps (--walk-step 5)."""
    t = int(target)
    pm = t if t > 100 else t * 10  # target in permille
    out = [str(pct) for pct in range(5, 11) if pct * 10 < pm]
    out += [str(v) for v in range(110, pm, step_pm)]
    return out
# Bench-command ramp through the AM32 sine-start band (percent, seconds).
SINE_RAMP = [(1, 2.0), (2, 0.5), (3, 0.5), (4, 0.5)]
KV = re.compile(r"(\w+)=(-?\d+)")


def _terminate_as_interrupt(signum, frame):  # noqa: ARG001
    # A terminated motor script must still run its `finally` kill path
    # (Python's default SIGTERM handling skips it — found when an external
    # `timeout` cut a 50 % restart test mid-drive).
    raise KeyboardInterrupt(f"signal {signum}")


class Bench:
    def __init__(self, port: str, log: pathlib.Path) -> None:
        import signal

        for name in ("SIGTERM", "SIGBREAK", "SIGINT"):
            if hasattr(signal, name):
                signal.signal(getattr(signal, name), _terminate_as_interrupt)
        self.s = serial.Serial(port, BAUD, timeout=0.02)
        self.log = log.open("ab")
        self.buf = b""
        self.lines: list[str] = []
        self.phase = "idle"
        self.t_phase = time.time()

    def send(self, cmd: str) -> None:
        self.s.write((cmd + "\n").encode())
        self.log.write(f">>> {cmd}\n".encode())

    def pump(self, secs: float = 0.0) -> None:
        end = time.time() + secs
        while True:
            b = self.s.read(4096)
            if b:
                self.log.write(b)
                self.buf += b
                *done, self.buf = self.buf.split(b"\n")
                for ln in done:
                    self.lines.append(ln.decode("latin-1").strip())
            if time.time() >= end:
                break

    def query(self, keep: str = "") -> dict[str, dict[str, int]]:
        """Send 'i' (repeating `keep` as the setpoint stream), return the
        parsed i/b lines."""
        self.lines.clear()
        self.send("i")
        got: dict[str, dict[str, int]] = {}
        end = time.time() + 1.0
        while time.time() < end and not ("i" in got and "b" in got):
            self.pump(0.05)
            for ln in self.lines:
                if ln.startswith("i step="):
                    got["i"] = {k: int(v) for k, v in KV.findall(ln)}
                elif ln.startswith("b vbus_mv="):
                    got["b"] = {k: int(v) for k, v in KV.findall(ln)}
                elif ln.startswith("r duty="):
                    got["r"] = {k: int(v) for k, v in KV.findall(ln)}
                elif "BENCH KILL" in ln:
                    got["kill"] = {"line": 1}
                    print(f"   !! {ln}")
        if "i" not in got or "b" not in got:
            raise RuntimeError("no i/b reply from firmware")
        return got

    def stream(self, value: str, secs: float) -> None:
        """Send the setpoint at 10 Hz (two identical sends commit)."""
        end = time.time() + secs
        while time.time() < end:
            self.send(value)
            self.pump(0.1)
            for ln in self.lines:
                if "BENCH KILL" in ln:
                    raise RuntimeError(
                        f"firmware guard in '{self.phase}' "
                        f"+{time.time() - self.t_phase:.2f}s: {ln}"
                    )
                if "last reset:" in ln or "entering main loop" in ln:
                    # The firmware rebooted mid-run (e.g. IWDG): stop at once —
                    # never keep streaming a drive command into a fresh boot.
                    raise RuntimeError(
                        f"firmware REBOOTED in '{self.phase}' "
                        f"+{time.time() - self.t_phase:.2f}s: {ln}"
                    )
            self.lines.clear()

    def kill(self) -> None:
        for _ in range(3):
            try:
                self.send("w")
                self.pump(0.05)
            except Exception:  # noqa: BLE001 — must not mask the kill path
                pass

    def close(self) -> None:
        self.log.close()
        self.s.close()


def summary(tag: str, q: dict[str, dict[str, int]]) -> str:
    i, b = q["i"], q["b"]
    return (
        f"{tag}: run={i['run']} old={i['old']} duty={i['duty']} avg={i['avg']} "
        f"ehz={b['ehz']} vbus_mv={b['vbus_mv']} i_ma={b['i_ma']} imean_ma={b['imean_ma']}"
        f"(n={b['in']}) nf={b['nf']} en={b['en']} moe={b['moe']} armed={b['armed']} "
        f"zero={b['zero']} killed={b['killed']}"
    )


# firmware50 reference on this bench at 12.0 V (effective duty %, eHz, mA
# metered-calibrated — same calibration the rm32 3-shunt path uses).
FW50_REF = [(10.0, 388, 73), (15.0, 697, 154)]
FW50_REF_MV = 12_000


def fw50_expect(duty_pct: float, vbus_mv: float) -> tuple[float, float]:
    """Linear interpolation (extrapolation refused beyond ±2 % duty) of the
    firmware50 table; eHz scaled by bus voltage (BEMF-limited speed ∝ V·d)."""
    (d0, e0, i0), (d1, e1, i1) = FW50_REF
    if not d0 - 2 <= duty_pct <= d1 + 2:
        raise ValueError(f"duty {duty_pct:.1f}% outside the firmware50 reference span")
    t = (duty_pct - d0) / (d1 - d0)
    return (e0 + t * (e1 - e0)) * vbus_mv / FW50_REF_MV, i0 + t * (i1 - i0)


def compare_fw50(closed: list[dict[str, dict[str, int]]]) -> bool:
    n = len(closed)
    duty = sum(s["i"]["duty"] for s in closed) / n / 20.0  # per-2000 → %
    ehz = sum(s["b"]["ehz"] for s in closed) / n
    ima = sum(s["b"]["imean_ma"] for s in closed) / n
    vmv = sum(s["b"]["vbus_mv"] for s in closed) / n
    e_ref, i_ref = fw50_expect(duty, vmv)
    de, di = (ehz - e_ref) / e_ref, (ima - i_ref) / i_ref
    ok = abs(de) <= 0.10 and abs(di) <= 0.10
    print(
        f"vs firmware50 @ {duty:.1f}% / {vmv:.0f} mV: ehz {ehz:.0f} vs {e_ref:.0f} ({de:+.1%}), "
        f"i {ima:.0f} vs {i_ref:.0f} mA ({di:+.1%}) -> {'AGREE' if ok else 'DISAGREE'} (±10 %)"
    )
    return ok


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default=PORT)
    ap.add_argument("--duty", type=int, default=10, help="bench percent, <= 15")
    ap.add_argument("--rung", type=int, default=0,
                    help="APPLIED duty in tenths of a percent (climb rungs), <= MAX_RUNG_TENTHS")
    ap.add_argument("--hold", type=float, default=10.0)
    ap.add_argument("--arm", type=float, default=3.5, help="seconds of 0 before drive")
    ap.add_argument("--arm-only", action="store_true")
    ap.add_argument("--min-vbus", type=int, default=9000, help="refuse to drive below (mV)")
    ap.add_argument("--walk-step", type=int, default=10,
                    help="walk increment in permille per 1 s step (climb: 5)")
    ap.add_argument("--quiet", action="store_true", help="no queries from changeover to end of hold")
    ap.add_argument(
        "--no-sine-ramp",
        dest="sine_ramp",
        action="store_false",
        help="step straight to --duty (boards without sine start)",
    )
    ap.add_argument("--out", type=pathlib.Path)
    args = ap.parse_args()
    if args.rung:
        if args.rung > MAX_RUNG_TENTHS:
            raise SystemExit(f"rung {args.rung} refused (climb cap {MAX_RUNG_TENTHS})")
        cmd = str(rung_command(args.rung))
    else:
        if not 0 < args.duty <= MAX_DUTY:
            raise SystemExit(f"duty {args.duty}% refused (bring-up cap {MAX_DUTY}%)")
        cmd = str(args.duty)
    out = args.out or pathlib.Path("captures/binz") / time.strftime("spin_%Y%m%d_%H%M%S.txt")
    out.parent.mkdir(parents=True, exist_ok=True)

    bench = Bench(args.port, out)
    verdict = 1
    try:
        bench.pump(0.2)
        q0 = bench.query()
        print(summary("idle", q0))
        if q0["b"]["killed"]:
            raise RuntimeError("firmware guard already latched — reset the board first")
        rest = q0["b"]["vbus_mv"]

        print(f"arming: streaming 0 for {args.arm:.1f} s")
        bench.stream("0", args.arm)
        q1 = bench.query()
        print(summary("armed", q1))
        b = q1["b"]
        problems = []
        if not b["armed"]:
            problems.append("not armed")
        if not (b["en"] and b["moe"]):
            problems.append("gate driver / MOE not enabled while armed")
        if b["nf"]:
            problems.append("nFAULT asserted")
        if b["zero"] == 0:
            problems.append("no 3-shunt zero")
        if args.arm_only:
            print("arm-only: " + ("PASS" if not problems else "FAIL: " + ", ".join(problems)))
            verdict = 0 if not problems else 1
            return verdict
        if b["vbus_mv"] < args.min_vbus:
            problems.append(f"bus {b['vbus_mv']} mV < {args.min_vbus} mV (supply off?)")
        if problems:
            raise RuntimeError("precondition: " + ", ".join(problems))

        if args.sine_ramp:
            # AM32 sine start: bench 1 % = slow sine stepping, 2-4 % = the
            # fixed-rate pre-changeover band; >= 5 % changes over to six-step
            # immediately, so walk the band before the drive setpoint.
            for pct, secs in SINE_RAMP:
                print(f"sine ramp: {pct}% for {secs:.1f} s")
                bench.phase, bench.t_phase = f"sine {pct}%", time.time()
                bench.stream(str(pct), secs)
                q = bench.query()
                print(summary(f"  after {pct}%", q) + f" zc={q['i']['zc']} step={q['i']['step']}")
            # Changeover (5 %) then walk up 1 %/s: a step to the setpoint
            # accelerates the rotor hard enough to exceed the 500 mA kill.
            for pct in walk_commands(cmd, args.walk_step):
                bench.phase, bench.t_phase = f"walk {pct}%", time.time()
                bench.stream(str(pct), 1.0)
                if not args.quiet:
                    q = bench.query()
                    print(summary(f"  walk {pct}%", q) + f" zc={q['i']['zc']}")
                    if q["b"]["killed"] or "kill" in q:
                        raise RuntimeError("firmware guard latched during walk")
        print(f"drive: cmd {cmd} for {args.hold:.1f} s")
        t0 = time.time()
        bench.phase, bench.t_phase = f"hold {cmd}", t0
        samples = []
        started = False
        if args.quiet:
            # NO UART replies while the bridge drives: at 82.5 % a query's
            # TX coupled noise into the comparator and starved the main loop
            # (polled TX) into an IWDG reset. `H` silently resets the hold
            # aggregates; the firmware freezes the run's numbers on the stop
            # edge and prints them as an `r` line on the first query after.
            bench.send("H")
            bench.pump(0.05)
            bench.stream(cmd, args.hold)
            bench.send("0")
            bench.send("0")
            bench.pump(0.5)
            q = bench.query()
            r = q.get("r")
            if not r:
                raise RuntimeError("no hold snapshot (r line) after stop")
            print(f"t={time.time() - t0:4.1f}: hold duty={r['duty']} imean_ma={r['imean_ma']} "
                  f"i50max={r['i50max']} ecom10={r['ecom10']} "
                  f"ehz={1e7 / r['ecom10'] if r['ecom10'] else 0:.0f} ews={r['ews']} dsy={r['dsy']} "
                  f"vbus_after_mv={q['b']['vbus_mv']} killed={q['b']['killed']}")
            if q["b"]["killed"] or "kill" in q:
                raise RuntimeError("firmware guard latched during drive")
            started = r["ecom10"] > 0
        for _ in range(0 if args.quiet else 2):  # early look at the changeover / lock
            bench.stream(cmd, 0.5)
            q = bench.query()
            print(summary(f"t={time.time() - t0:4.1f}", q) + f" zc={q['i']['zc']}")
            if q["b"]["killed"] or "kill" in q:
                raise RuntimeError("firmware guard latched during drive")
        while not args.quiet and time.time() - t0 < args.hold:
            bench.stream(cmd, 2.0)
            q = bench.query()
            line = summary(f"t={time.time() - t0:4.1f}", q)
            print(line)
            samples.append(q)
            b = q["b"]
            if b["killed"] or "kill" in q:
                raise RuntimeError("firmware guard latched during drive")
            if b["nf"]:
                raise RuntimeError("nFAULT during drive")
            if rest and b["vbus_mv"] * 100 < rest * 90:
                raise RuntimeError(f"bus sag {b['vbus_mv']} vs rest {rest}")
            if q["i"]["run"]:
                started = True
            elif time.time() - t0 > 6.0:
                raise RuntimeError("motor not running 6 s after drive start")
            if started and not q["i"]["run"]:
                raise RuntimeError("motor stopped running during hold")
        bench.send("0")
        bench.send("0")
        bench.pump(0.3)
        bench.kill()
        bench.pump(1.0)
        qe = bench.query()
        print(summary("after stop", qe))
        closed = [s for s in samples if s["i"]["run"] and not s["i"]["old"] and s["b"]["ehz"] > 0]
        if closed:
            n = len(closed)
            print(
                f"closed-loop samples {n}/{len(samples)}: "
                f"ehz mean {sum(s['b']['ehz'] for s in closed) / n:.0f}, "
                f"imean mean {sum(s['b']['imean_ma'] for s in closed) / n:.0f} mA, "
                f"vbus mean {sum(s['b']['vbus_mv'] for s in closed) / n:.0f} mV"
            )
            try:
                compare_fw50(closed)
            except ValueError as e:
                print(f"vs firmware50: {e}")
        stopped_off = not qe["i"]["run"] and qe["i"]["duty"] == 0
        print("stop: " + ("outputs off" if stopped_off else "NOT CLEAN"))
        verdict = 0 if closed and stopped_off else 1
    except KeyboardInterrupt:
        print("interrupted")
    except Exception as e:  # noqa: BLE001 — every failure ends in the kill path
        print(f"ABORT: {e}")
    finally:
        bench.kill()
        bench.pump(0.3)
        bench.close()
        print(f"log: {out}")
    return verdict


if __name__ == "__main__":
    sys.exit(main())
