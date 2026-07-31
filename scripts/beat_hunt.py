#!/usr/bin/env python3
"""Beat hunt: 20s zctrace capture at 100% + spike-train analysis.

Per-commutation records (~19k/s, 0.5us resolution) — a ~100us clip is
a late window (thiszc spike) that CANNOT hide. Analysis: reconstruct
the time axis from cumulative thiszc, flag outlier windows, print the
spike times + inter-spike intervals — a regular ~2s beat shows as a
regular spike train. Also timestamps [loop] heartbeat positions in the
same stream for correlation.

Usage: beat_hunt.py [port] [pct] [secs]
"""
import struct
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
        b.cmd(b"Z", settle=0.2)      # trace ON
        mark = len(b.buf)
        print(f"== capturing {pct}% for {secs:.0f}s (trace on) ==")
        b.hold(pct, secs)
        b.cmd(b"Z", settle=0.3)      # trace OFF
        t0 = time.time()
        while time.time() - t0 < 2.0:
            b.buf.extend(b.p.read(400000))
            time.sleep(0.1)
        raw = bytes(b.buf[mark:])
    open("../captures/beat_hunt.bin", "wb").write(raw)
    print(f"captured {len(raw)} bytes")

    # Parse A9 rows with sanity bounds (phantom-record class).
    rows, i, n = [], 0, len(raw)
    loops = []  # byte offsets of [loop lines
    while i + 15 <= n:
        if raw[i] == 0x5B and raw[i+1] == 0xA9:
            fl = raw[i+2]
            v = struct.unpack_from("<6H", raw, i+3)
            if v[0] < 30000 and v[3] <= 2100:  # tz, duty sane
                rows.append((i, fl & 7, bool(fl & 0x80)) + v)
            i += 15
        elif raw[i] == 0x5B and raw[i+1] == 0xA6:
            i += 15
        else:
            if raw[i:i+6] == b"[loop ":
                loops.append(i)
            i += 1
    print(f"zct rows: {len(rows)}   [loop] lines: {len(loops)}")
    if len(rows) < 1000:
        print("too few rows — trace off or batching gap")
        return 1

    # Time axis: cumulative thiszc (0.5us ticks). Batching halves are
    # 50-on/50-off; time within recorded runs is exact, gaps are ~equal
    # — good enough for a 2s period hunt.
    tz = [r[3] for r in rows]
    med = sorted(tz)[len(tz)//2]
    t, ts = 0.0, []
    for v in tz:
        t += v / 2e6
        ts.append(t)
    total = ts[-1]
    # account: recorded-time vs wall secs => batching duty factor
    print(f"median tz={med} ({med/2:.1f}us)  recorded time={total:.1f}s "
          f"of {secs:.0f}s wall")

    # spikes: windows > 1.5x median
    spikes = [(ts[k], tz[k], rows[k][1]) for k in range(len(tz))
              if tz[k] > med * 1.5]
    print(f"late windows (>1.5x med): {len(spikes)}")
    for k, (st, v, step) in enumerate(spikes[:40]):
        gap = st - spikes[k-1][0] if k else 0
        print(f"  t={st:6.2f}s  tz={v:>5} ({v/2:.0f}us, {v/med:.1f}x) "
              f"step={step}  d_prev={gap:5.2f}s")
    if len(spikes) > 2:
        gaps = [spikes[k][0] - spikes[k-1][0] for k in range(1, len(spikes))]
        gm = sorted(gaps)[len(gaps)//2]
        print(f"inter-spike median: {gm:.2f}s "
              f"(regular ~2s beat would show here)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
