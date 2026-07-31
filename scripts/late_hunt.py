#!/usr/bin/env python3
"""Late-window beat hunt: quiet 100% hold, then dump the onboard
late-window event log ('L') and print inter-event spacing — the
drop-proof answer to "is the ~2s beat real and periodic".

Usage: late_hunt.py [port] [pct] [secs]
"""
import re
import sys
import time

from bench_lib import Bench, reset_board


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    pct = int(sys.argv[2]) if len(sys.argv) > 2 else 100
    secs = float(sys.argv[3]) if len(sys.argv) > 3 else 20.0
    identity, _ = reset_board(port)
    if identity != "rm32":
        print(f"ABORT identity={identity}")
        return 1
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            print("engage failed")
            return 1
        b.cmd(b"T", settle=0.25)
        for p in (70, 80, 90, 96, pct):
            if p <= pct:
                b.hold(p, 2.2)
        print(f"== quiet hold {pct}% x{secs:.0f}s (listen & count clips) ==")
        t_start = time.time()
        b.hold(pct, secs)
        hold_end = time.time() - t_start
        # dump the ring
        n0 = len(b.buf)
        b.cmd(b"L", settle=1.0)
        t0 = time.time()
        while time.time() - t0 < 2.0:
            b.buf.extend(b.p.read(200000))
            if b"LW END" in bytes(b.buf[n0:]):
                break
            time.sleep(0.1)
        seg = "".join(chr(x) if 32 <= x < 127 or x == 10 else "."
                      for x in b.buf[n0:])
        post = b.info()
    events = []
    for m in re.finditer(r"^LW (\d+) (\d+)$", seg, re.M):
        events.append((int(m.group(1)) / 1000.0, int(m.group(2))))
    total = re.search(r"LW n=(\d+)", seg)
    print(f"hold {hold_end:.1f}s   post: {post}")
    print(f"late-window events (lifetime n={total.group(1) if total else '?'}, "
          f"ring holds last {len(events)}):")
    for k, (t, tz) in enumerate(events):
        gap = t - events[k-1][0] if k else 0
        print(f"  t={t:8.3f}s  tz={tz:>5} ({tz/2:.0f}us)  d_prev={gap:6.3f}s")
    if len(events) > 2:
        gaps = sorted(t2 - t1 for (t1, _), (t2, _) in zip(events, events[1:]))
        print(f"\ninter-event gaps: median {gaps[len(gaps)//2]:.3f}s  "
              f"min {gaps[0]:.3f}  max {gaps[-1]:.3f}")
        print("regular ~2s beat => median ~2.0 with low spread")
    elif not events:
        print("  NONE — no late windows at all during the hold")
    return 0


if __name__ == "__main__":
    sys.exit(main())
