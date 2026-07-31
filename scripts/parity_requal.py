#!/usr/bin/env python3
"""rm32 battery re-qual — mirror of the clone's parity reference run.

Segments (clone protocol):
  A) 10% ladder: 10->100%, 5s dwells
  B) 2% ladder:  10->100->10%, 91 rungs, 1.5s dwells
  C) slams:      10->100 and 100->10, x2 each

Metrics (onboard, drop-proof — the clone's quantities):
  exc = locked commutations with excursion >25% vs rolling mean (e_com/3)
  wex = worst excursion (permille), cm = locked commutations counted
  dsy = desync events; kills/reboots watched

Poll law: NO info polls at rungs >= 96% (poll-print disturbance); reads
happen at safe rungs / segment boundaries. Verbatim detector, no T.

Output: captures/requal.json + console summary.
"""
import json
import sys
import time

from bench_lib import Bench, reset_board


def snap(b):
    inf = b.info()
    if inf is None:
        return None
    return {
        "t": time.time(), "ci": inf.ci, "duty": inf.duty, "zc": inf.zc,
        "amps": inf.amps, "volts": inf.volts, "dsy": inf.dsy,
        "exc": inf.raw.get("exc"), "wex": inf.raw.get("wex"),
        "cm": inf.raw.get("cm"), "running": inf.running,
    }


def num(s, k):
    return int(s[k]) if s and s.get(k) is not None else 0


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    identity, _ = reset_board(port)
    if identity != "rm32":
        print(f"ABORT identity={identity}")
        return 1
    out = {"segments": [], "samples": []}
    with Bench(port) as b:
        if not b.engage_from_stop(30):
            print("engage failed")
            return 1
        base = snap(b)
        out["samples"].append(base)
        print(f"engaged: {base}")

        def seg_run(name, profile):
            """profile = list of (pct, dwell). Polls only at pct<96."""
            start = snap(b)
            for pct, dwell in profile:
                b.hold(pct, dwell)
                if pct < 96:
                    s = snap(b)
                    if s:
                        s["pct"] = pct
                        out["samples"].append(s)
            b.hold(30, 1.5)  # safe rung for the closing read
            end = snap(b)
            seg = {
                "name": name,
                "comms": num(end, "cm") - num(start, "cm"),
                "exc": num(end, "exc") - num(start, "exc"),
                "wex_glob": num(end, "wex"),
                "dsy": num(end, "dsy") - num(start, "dsy"),
                "v_end": end["volts"] if end else None,
                "reboots": len(b.reboot_events),
                "kill": b.kill_line_seen,
            }
            seg["exc_per_1k"] = round(seg["exc"] * 1000 / seg["comms"], 2) \
                if seg["comms"] else None
            out["segments"].append(seg)
            print(f"  == {name}: {seg}")

        # A) 10% ladder
        print("== A: 10% ladder ==")
        seg_run("10pct-ladder", [(p, 5.0) for p in range(10, 101, 10)])

        # B) 2% ladder up+down
        print("== B: 2% ladder (91 rungs) ==")
        rungs = list(range(10, 101, 2)) + list(range(98, 9, -2))
        seg_run("2pct-ladder", [(p, 1.5) for p in rungs])

        # C) slams x2 each direction
        print("== C: slams ==")
        slam_profile = []
        for _ in range(2):
            slam_profile += [(10, 2.0), (100, 3.0), (10, 2.0)]
        seg_run("slams", slam_profile)

        final = snap(b)
        out["samples"].append(final)
        print(f"final: {final}")
    with open("../captures/requal.json", "w") as f:
        json.dump(out, f, indent=1)
    print("\n== rm32 battery re-qual summary ==")
    for s in out["segments"]:
        print(f"  {s['name']:12s} comms={s['comms']:>7} exc>25%={s['exc']}"
              f" ({s['exc_per_1k']}/1k) dsy={s['dsy']}"
              f" reboots={s['reboots']} kill={s['kill']}")
    print(f"  worst excursion (global): "
          f"{out['segments'][-1]['wex_glob']/10:.1f}%")
    return 0


if __name__ == "__main__":
    sys.exit(main())
