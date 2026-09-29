#!/usr/bin/env python3
"""ENV-26: effective commutation angle from the FINE stamps only.

Service rows' coarse `at_us` is `sched_raw` as logged AFTER dispatch, so when a commutation
also arms the blanking floor (purpose 3) it holds the blank-end schedule, not the
commutation instant (ENV-25's angle table used it and is superseded). Here:
  crossing   = accept row at_fine          (TIM2, 125 ns, 16-bit)
  bridge     = purpose-1 service row x_fine (the actual bridge write)
Each ring is unwrapped separately; they are aligned on their final rows, which are within
one 8.192 ms wrap of each other at the freeze (asserted)."""
import sys, statistics as st
TICK = 0.125
def unwrap(v):
    out, b, p = [], 0, None
    for x in v:
        if p is not None and x < p - 32768: b += 65536
        out.append(x + b); p = x
    return out
for path in sys.argv[1:]:
    R = [list(map(int, l.split()[1:])) for l in open(path) if l.startswith("CHAIN ")]
    A = [r for r in R if r[0] == 1]; S = [r for r in R if r[0] == 2 and (r[7] & 0x7F) == 1 and r[3] != 0]
    ca = unwrap([a[2] for a in A]); cs = unwrap([s[3] for s in S])
    # align: shift service axis by whole wraps so its last bridge sits within +-4 ms of the last crossing
    d = cs[-1] - ca[-1]; k = round(d / 65536); cs = [x - k * 65536 for x in cs]
    assert abs(cs[-1] - ca[-1]) * TICK < 4096, "rings not alignable"
    rows = []
    j = 0
    for i in range(1, len(ca)):
        while j < len(cs) and cs[j] <= ca[i]: j += 1
        if j >= len(cs): break
        iv = (ca[i] - ca[i-1]) * TICK
        lag = (cs[j] - ca[i]) * TICK
        rows.append(((ca[-1] - ca[i]) * TICK, A[i][6], iv, lag, lag / iv, A[i][4]))
    base = [r[4] for r in rows[:-40]]
    print(f"{path.split('/')[-1]}: angle median {st.median(base):.3f} p5 {sorted(base)[len(base)//20]:.3f} p95 {sorted(base)[19*len(base)//20]:.3f}")
    out = [r for r in rows if r[4] > 0.45 or r[4] < 0.10]
    print(f"  angle outliers (>0.45 or <0.10): {len(out)} of {len(rows)}")
    for r in out[-10:]:
        print(f"    {r[0]:7.0f} us before end  s{r[1]} interval {r[2]:5.1f} bridge-after-crossing {r[3]:5.1f} angle {r[4]:.2f} wait {r[5]}")
    print("  last 12:", " ".join(f"{r[4]:.2f}" for r in rows[-12:]))
