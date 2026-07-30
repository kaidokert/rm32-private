#!/usr/bin/env python3
"""Comp-deadlock healer test — the void-initiator verdict run.

The healer (handle_tim6) detects Running+interrupt-mode+EXTI-masked+
no-commutation-scheduled — the deadlock the void autopsy measured — and
re-enables within one tick, counting HEAL_N. Prediction if the masked
deadlock IS the initiator: heal= clicks at 100%, the 22.5 ms voids and
their desync cascades vanish, and 100% HOLDS.
"""
import re
import sys
import time

from bench_lib import Bench, reset_board


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    hold_secs = float(sys.argv[2]) if len(sys.argv) > 2 else 10.0
    identity, causes = reset_board(port)
    if identity != "rm32":
        print(f"ABORT: identity={identity} causes={causes}")
        return 1
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            print("engage failed: " + b.last_engage_report)
            return 1
        for pct in (70, 80, 90, 93, 96):
            b.hold(pct, 2.2)
        print(f"96%: {b.info()}")
        t0 = time.time()
        while time.time() - t0 < hold_secs:
            b.hold(100, 1.0)
            n = len(b.buf)
            inf = b.info()
            seg = "".join(chr(x) if 32 <= x < 127 or x == 10 else "."
                          for x in b.buf[n:])
            void = re.search(r"\[void [^\]]+\]", seg)
            print(f"[100 t={time.time()-t0:4.1f}] {inf}"
                  + (f"  {void.group(0)}" if void else ""))
            if inf and inf.dsy > 400:
                print(">> storm — healer did not prevent the cascade")
                break
    print("killed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
