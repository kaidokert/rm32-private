#!/usr/bin/env python3
"""THE observer test: verbatim detector, no J, 100% hold with ZERO info
polls (pure throttle stream) — dsy read once at the end.

Every storm observation in the campaign involved mid-hold info polling;
every poll-free hold was clean (relaxed-T). If dsy stays ~0 here on the
VERBATIM detector, the storms are poll-print-induced (the i-line dprintln
from main disturbs the accept stream) and the wall itself was observer
artifact. If dsy is high, the bursts are intrinsic.

Usage: noopoll_verbatim.py [port] [secs]
"""
import sys
import time

from bench_lib import Bench, reset_board


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    secs = float(sys.argv[2]) if len(sys.argv) > 2 else 12.0
    identity, _ = reset_board(port)
    if identity != "rm32":
        print(f"ABORT identity={identity}")
        return 1
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            print("engage failed")
            return 1
        # verbatim detector, no J, no T
        for p in (70, 80, 90, 93, 96):
            b.hold(p, 2.2)
        pre = b.info()
        print(f"96% (last poll before silence): {pre}")
        print(f"== {secs:.0f}s at 100%, ZERO polls ==")
        b.hold(100, secs)
        post = b.info()
        print(f"post: {post}")
        if pre and post:
            print(f"\ndsy delta over the silent hold: {post.dsy - pre.dsy}")
            print("~0  => storms are POLL-INDUCED (observer artifact)")
            print(">50 => bursts are intrinsic")
    print("killed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
