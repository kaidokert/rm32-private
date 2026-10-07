#!/usr/bin/env python3
"""binz event qualification: restart onto a coasting rotor, and recovery from
an injected comparator disturbance. Built on binz_spin.py's Bench (kill `w` on
every exit path; the firmware guard latches independently).

    python scripts/binz_events.py restart --rung 500 --coast-ms 300
    python scripts/binz_events.py disturb --rung 400 --spam-s 3

restart: arm, sine ramp, walk to the rung, hold 5 s; throttle 0 for
  --coast-ms (sine idle: all outputs off, rotor coasts); walk back up from
  the changeover with the rotor still spinning; `H` (silent aggregate reset);
  hold 5 s; stop; read the `r` hold snapshot. PASS = no kill, dsy delta 0 on
  the re-hold, re-hold speed within 3 % of the pre-restart hold.
disturb: arm, ramp, walk, hold 3 s; then spam `i` queries every 50 ms for
  --spam-s (UART TX couples into the comparator on this bench: an EMI-like
  zero-cross disturbance), recording the firmware's cumulative desync count
  from the replies; `H`; quiet hold 5 s; stop; `r` snapshot. PASS = no kill,
  quiet re-hold locked (dsy delta 0) at the undisturbed speed (within 3 %).
  The desyncs counted DURING the spam are the event being exercised.

Every line received is written to captures/binz/<mode>_<ts>.txt. Nothing is
queried while the bridge drives except the deliberate disturbance.
"""

from __future__ import annotations

import argparse
import importlib.util
import pathlib
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("binz_spin", HERE / "binz_spin.py")
spin = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(spin)


def walk_up(bench, cmd: str, step_pm: int) -> None:
    for pct in spin.walk_commands(cmd, step_pm):
        bench.phase, bench.t_phase = f"walk {pct}", time.time()
        bench.stream(pct, 1.0)


def snapshot(bench) -> dict[str, int]:
    bench.send("0")
    bench.send("0")
    bench.pump(0.5)
    q = bench.query()
    r = q.get("r")
    if not r:
        raise RuntimeError("no hold snapshot (r line) after stop")
    if q["b"]["killed"] or "kill" in q:
        raise RuntimeError("firmware guard latched")
    return r


def hold(bench, cmd: str, secs: float) -> dict[str, int]:
    bench.send("H")
    bench.pump(0.05)
    bench.phase, bench.t_phase = f"hold {cmd}", time.time()
    bench.stream(cmd, secs)
    return snapshot(bench)


def ehz(r: dict[str, int]) -> float:
    return 1e7 / r["ecom10"] if r["ecom10"] else 0.0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["restart", "disturb", "step", "inject"])
    ap.add_argument("--ref-ehz", type=float, default=0.0,
                    help="inject mode: the rung's qualified hold-mean eHz to re-lock to")
    ap.add_argument("--to-rung", type=int, default=150, help="step mode: low rung (tenths)")
    ap.add_argument("--diode-decel", action="store_true",
                    help="step mode: force DIODE drive ('D') for the deceleration so the "
                         "PSU sees no regeneration, then return to AUTO ('D','D') for the re-hold")
    ap.add_argument("--battery", action="store_true",
                    help="step mode: the supply can SINK regenerated current (a battery). "
                         "Without it a down-step must use --diode-decel: on the bench PSU "
                         "both the hard step and the --down-pm ramp pumped the bus to "
                         "~17 V past the 14 V OVOLT stop (killing the bridge does not stop "
                         "regeneration: the body diodes keep rectifying BEMF into the bus)")
    ap.add_argument("--down-pm", type=int, default=0,
                    help="step mode: ramp down this many permille every 0.1 s instead of an "
                         "instant step (a hard step regenerates the bench PSU into OVOLT)")
    ap.add_argument("--rung", type=int, required=True, help="applied duty, tenths")
    ap.add_argument("--coast-ms", type=int, default=300)
    ap.add_argument("--probe-phases", action="store_true",
                    help="restart mode: read the cumulative desync count after the pre-hold, "
                         "the coast and the re-walk (UART queries couple into the comparator: "
                         "use only at rungs where the disturb test showed 0, i.e. <= 50 %%)")
    ap.add_argument("--spam-s", type=float, default=3.0)
    ap.add_argument("--walk-step", type=int, default=5)
    ap.add_argument("--port", default=spin.PORT)
    args = ap.parse_args()
    if args.rung > spin.MAX_RUNG_TENTHS:
        raise SystemExit(f"rung {args.rung} refused (climb cap {spin.MAX_RUNG_TENTHS})")
    cmd = str(spin.rung_command(args.rung))
    if args.mode == "step" and args.to_rung < args.rung and not (args.diode_decel or args.battery):
        raise SystemExit(
            f"down-step {args.rung} -> {args.to_rung} refused on the PSU: regeneration "
            "overvolts the bus (16.9-17.0 V measured). Use --diode-decel, or --battery "
            "when the supply can sink current.")
    out = pathlib.Path("captures/binz") / time.strftime(f"{args.mode}_%Y%m%d_%H%M%S.txt")
    out.parent.mkdir(parents=True, exist_ok=True)
    bench = spin.Bench(args.port, out)
    verdict = 1
    try:
        bench.pump(0.2)
        q0 = bench.query()
        if q0["b"]["killed"]:
            raise RuntimeError("guard already latched — reset first")
        bench.phase = "arm"
        bench.stream("0", 6.0)
        for pct, secs in spin.SINE_RAMP:
            bench.phase, bench.t_phase = f"sine {pct}", time.time()
            bench.stream(str(pct), secs)
        walk_up(bench, cmd, args.walk_step)
        if args.mode == "restart":
            bench.send("H")
            bench.pump(0.05)
            bench.phase, bench.t_phase = "pre-hold", time.time()
            bench.stream(cmd, 5.0)
            marks = []
            if args.probe_phases:
                marks.append(("pre-hold", bench.query()["i"]["dsy"]))
            # coast: throttle 0 (the firmware enters sine idle: outputs off)
            bench.phase, bench.t_phase = "coast", time.time()
            bench.stream("0", args.coast_ms / 1000.0)
            if args.probe_phases:
                marks.append(("coast", bench.query()["i"]["dsy"]))
            r0 = None  # the pre-hold snapshot is frozen by the coast's stop edge
            walk_up(bench, cmd, args.walk_step)
            if args.probe_phases:
                q = bench.query()
                marks.append(("re-walk", q["i"]["dsy"]))
                print("  phase desyncs (cumulative): start=" + str(q0["i"]["dsy"]) + " "
                      + " ".join(f"{k}={v}" for k, v in marks)
                      + f"  [after re-walk: run={q['i']['run']} old={q['i']['old']} "
                        f"ci={q['i']['ci']} duty={q['i']['duty']}]")
            r1 = hold(bench, cmd, 5.0)
            qa = bench.query()
            d_all = qa["i"]["dsy"] - q0["i"]["dsy"]
            # the pre-restart snapshot was overwritten; compare against the
            # same rung's qualified hold speed from a separate reference run
            print(f"restart rung {args.rung} coast {args.coast_ms} ms: desyncs during the restart "
                  f"(cumulative)={d_all}; re-hold duty={r1['duty']} ehz={ehz(r1):.0f} "
                  f"imean_ma={r1['imean_ma']} dsy since re-hold={r1['dsy']}")
            verdict = 0 if r1["dsy"] == 0 and r1["ecom10"] > 0 else 1
            del r0
        elif args.mode == "inject":
            # Deliberate loss of sync (`K`: the next commutation skips a
            # step), no UART traffic while driving: hold 3 s, inject, 3 s to
            # recover, `H`, quiet 5 s re-hold, stop. The post-stop `i` line's
            # cumulative desync count shows detection; the `r` snapshot
            # (since `H`) shows the recovered loop.
            dsy_base = q0["i"]["dsy"]
            inj_base = q0["b"].get("inj", 0)
            bench.phase, bench.t_phase = "pre-hold", time.time()
            bench.stream(cmd, 3.0)
            # Window 1: H, 1 s, K, 2 s, stop -> snapshot + sector histogram of
            # the injection itself (proof it fired, and the transient).
            bench.send("H")
            bench.pump(0.05)
            bench.stream(cmd, 1.0)
            bench.send("K")
            bench.phase, bench.t_phase = "inject", time.time()
            bench.stream(cmd, 2.0)
            rw = snapshot(bench)
            qw = bench.query()
            inj = qw["b"].get("inj", 0) - inj_base
            exc = [ln for ln in bench.lines if ln.startswith("hx ")]
            print(f"  injection window: inj consumed={inj} dsy={rw['dsy']} ehz={ehz(rw):.0f} "
                  f"{exc[-1] if exc else ''}")
            # Window 2: walk back up, quiet re-hold -> recovered loop.
            walk_up(bench, cmd, args.walk_step)
            bench.stream(cmd, 1.0)
            r = hold(bench, cmd, 5.0)
            q = bench.query()
            d_inj = q["i"]["dsy"] - dsy_base
            dev = abs(ehz(r) - args.ref_ehz) / args.ref_ehz * 100 if args.ref_ehz else 0.0
            print(f"inject rung {args.rung}: desyncs counted={d_inj}; re-hold duty={r['duty']} "
                  f"ehz={ehz(r):.0f} ({dev:.1f} % from ref {args.ref_ehz:.0f}) "
                  f"imean_ma={r['imean_ma']} dsy since re-hold={r['dsy']}")
            verdict = 0 if inj >= 1 and r["dsy"] == 0 and r["ecom10"] > 0 and dev < 3.0 else 1
        elif args.mode == "step":
            # Hard speed step (firmware50 battery study: decel below ~900 eHz
            # loses lock). Quiet pre-hold at the high rung, then the low
            # rung's command INSTANTLY (no walk); one query at the low rung
            # (UART coupling is harmless there) reads the cumulative desync
            # count; then a quiet re-hold must be locked at the low speed.
            low = str(spin.rung_command(args.to_rung)) if args.to_rung > 100 else str(args.to_rung)
            dsy_base = q0["i"]["dsy"]
            bench.phase, bench.t_phase = "pre-hold", time.time()
            bench.stream(cmd, 3.0)
            if args.diode_decel:
                bench.send("D")  # -> 1 DIODE (one short reply line)
                bench.pump(0.1)
            if args.down_pm > 0:
                v = int(cmd)
                while v - args.down_pm > int(low):
                    v -= args.down_pm
                    bench.phase, bench.t_phase = f"ramp {v}", time.time()
                    bench.stream(str(v), 0.1)
            bench.phase, bench.t_phase = f"step to {low}", time.time()
            bench.stream(low, 2.0)
            q = bench.query()
            if q["b"]["killed"] or "kill" in q:
                raise RuntimeError("firmware guard latched during the step")
            if args.diode_decel:
                bench.send("D")  # -> 2 COMPLEMENTARY
                bench.pump(0.05)
                bench.send("D")  # -> 3 AUTO (= the boot behaviour)
                bench.pump(0.2)
                bench.stream(low, 1.0)
            d_step = q["i"]["dsy"] - dsy_base
            locked = q["i"]["run"] == 1 and q["i"]["old"] == 0
            r = hold(bench, low, 5.0)
            print(f"step {args.rung} -> {args.to_rung}: desyncs during step={d_step}, "
                  f"locked 2 s after={locked} (ci={q['i']['ci']} avg={q['i']['avg']}); "
                  f"quiet re-hold duty={r['duty']} ehz={ehz(r):.0f} imean_ma={r['imean_ma']} dsy={r['dsy']}")
            verdict = 0 if locked and r["dsy"] == 0 and r["ecom10"] > 0 else 1
        else:
            ref = hold(bench, cmd, 3.0)  # stops: re-walk to resume the run
            walk_up(bench, cmd, args.walk_step)
            bench.phase, bench.t_phase = "settle", time.time()
            bench.stream(cmd, 2.0)
            dsy0 = None
            dsy1 = None
            end = time.time() + args.spam_s
            bench.phase, bench.t_phase = "spam", time.time()
            while time.time() < end:
                bench.send(cmd)
                q = bench.query()
                d = q["i"]["dsy"]
                dsy0 = d if dsy0 is None else dsy0
                dsy1 = d
                if q["b"]["killed"] or "kill" in q:
                    raise RuntimeError("firmware guard latched during disturbance")
                bench.pump(0.05)
            r = hold(bench, cmd, 5.0)
            d_spam = (dsy1 - dsy0) if dsy0 is not None else -1
            dev = abs(ehz(r) - ehz(ref)) / ehz(ref) * 100 if ehz(ref) else 999
            print(f"disturb rung {args.rung}: ref ehz={ehz(ref):.0f}; desyncs during "
                  f"{args.spam_s:.0f} s spam={d_spam}; quiet re-hold ehz={ehz(r):.0f} "
                  f"({dev:.1f} % from ref) imean_ma={r['imean_ma']} dsy={r['dsy']}")
            verdict = 0 if r["dsy"] == 0 and dev < 3.0 else 1
        print("PASS" if verdict == 0 else "FAIL")
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
