#!/usr/bin/env python3
"""Poll-free 2% ladder: 10->100->10, 1.5s dwells, ZERO polls during.
Reads counters only at start/end (30% safe rung). Discriminates:
transition desyncs (real) vs poll-injected ones.
"""
import sys
import time

from bench_lib import Bench, reset_board


def snap(b):
    inf = b.info()
    return inf


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    identity, _ = reset_board(port)
    if identity != "rm32":
        print(f"ABORT identity={identity}")
        return 1
    with Bench(port) as b:
        if not b.engage_from_stop(30):
            print("engage failed")
            return 1
        s0 = snap(b)
        print(f"start: {s0}  exc={s0.raw.get('exc')} cm={s0.raw.get('cm')}")
        print("== 2% ladder 10->100->10, POLL-FREE ==")
        for p in list(range(10, 101, 2)) + list(range(98, 9, -2)):
            b.hold(p, 1.5)
        b.hold(30, 1.5)
        s1 = snap(b)
        print(f"end:   {s1}  exc={s1.raw.get('exc')} cm={s1.raw.get('cm')}")
        cm = int(s1.raw.get("cm", 0)) - int(s0.raw.get("cm", 0))
        exc = int(s1.raw.get("exc", 0)) - int(s0.raw.get("exc", 0))
        print(f"\ncomms={cm} exc>25%={exc} ({exc*1000/cm:.2f}/1k) "
              f"dsy={s1.dsy - s0.dsy} wex={int(s1.raw.get('wex',0))/10:.1f}% "
              f"reboots={len(b.reboot_events)} kill={b.kill_line_seen}")
        print("(polled version: dsy=31, 1.13/1k; clone reference: dsy~2)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
