#!/usr/bin/env python3
"""Poll-free rm32 battery re-qual — the parity run.

All three clone-reference segments with ZERO polls during measurement;
counters read only at 30% boundaries. Saves captures/requal_quiet.json.
"""
import json
import sys
import time

from bench_lib import Bench, reset_board


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    identity, _ = reset_board(port)
    if identity != "rm32":
        print(f"ABORT identity={identity}")
        return 1
    out = {"segments": []}
    with Bench(port) as b:
        if not b.engage_from_stop(30):
            print("engage failed")
            return 1

        def boundary():
            inf = b.info()
            return {
                "cm": int(inf.raw.get("cm", 0)), "exc": int(inf.raw.get("exc", 0)),
                "wex": int(inf.raw.get("wex", 0)), "dsy": inf.dsy,
                "volts": inf.volts, "amps": inf.amps, "ci": inf.ci,
            }

        def seg(name, profile):
            s0 = boundary()
            t0 = time.time()
            for pct, dwell in profile:
                b.hold(pct, dwell)
            b.hold(30, 1.5)
            s1 = boundary()
            r = {
                "name": name, "secs": round(time.time() - t0, 1),
                "comms": s1["cm"] - s0["cm"], "exc": s1["exc"] - s0["exc"],
                "dsy": s1["dsy"] - s0["dsy"], "wex_glob": s1["wex"],
                "v_end": s1["volts"], "reboots": len(b.reboot_events),
                "kill": b.kill_line_seen,
            }
            r["exc_per_1k"] = round(r["exc"] * 1000 / r["comms"], 3) \
                if r["comms"] else None
            out["segments"].append(r)
            print(f"  {name}: {r}")

        print("== A: 10% ladder (poll-free) ==")
        seg("10pct-ladder", [(p, 5.0) for p in range(10, 101, 10)])
        print("== B: 2% ladder x91 (poll-free) ==")
        seg("2pct-ladder",
            [(p, 1.5) for p in list(range(10, 101, 2)) + list(range(98, 9, -2))])
        print("== C: slams x2 each way (poll-free) ==")
        seg("slams", [(10, 2.0), (100, 3.0), (10, 2.0),
                      (10, 0.5), (100, 3.0), (10, 2.0)])
        # top-speed check, single post-hold read
        b.hold(100, 3.0)
        b.hold(30, 1.0)
        top = boundary()
        out["top"] = top
        print(f"  post-100% boundary: {top}")
    with open("../captures/requal_quiet.json", "w") as f:
        json.dump(out, f, indent=1)
    print("\n== rm32 POLL-FREE battery re-qual ==")
    for s in out["segments"]:
        print(f"  {s['name']:12s} comms={s['comms']:>8} exc>25%={s['exc']} "
              f"({s['exc_per_1k']}/1k) dsy={s['dsy']} "
              f"reboots={s['reboots']} kill={s['kill']}")
    print(f"  global worst excursion: {out['segments'][-1]['wex_glob']/10:.1f}%")
    return 0


if __name__ == "__main__":
    sys.exit(main())
