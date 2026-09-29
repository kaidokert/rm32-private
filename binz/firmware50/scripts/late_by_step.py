#!/usr/bin/env python3
"""ENV-31: late-accept rate per logical step, from chain captures (the rotation discriminator's
metric). Late = accept interval > 1.25 x the capture's mean interval (the independent review's
definition). Prints one row per capture and a pooled row. Refuses captures with < 100 accepts."""
import sys, statistics as st
pool = {s: [0, 0] for s in range(1, 7)}
for path in sys.argv[1:]:
    A = [list(map(int, l.split()[1:])) for l in open(path) if l.startswith("CHAIN ") and l.split()[1] == "1"]
    assert len(A) >= 100, f"REFUSED {path}: {len(A)} accepts"
    iv = [((q[1] - p[1]) & 0xFFFF, q[6]) for p, q in zip(A, A[1:])]
    m = st.mean(x for x, _ in iv)
    per = {s: [0, 0] for s in range(1, 7)}
    for x, s in iv:
        per[s][1] += 1
        if x > 1.25 * m:
            per[s][0] += 1
    for s in per:
        pool[s][0] += per[s][0]; pool[s][1] += per[s][1]
    print(f"{path.split('/')[-1][:24]:24s}", " ".join(f"s{s}:{100*v[0]/max(1,v[1]):4.1f}%" for s, v in per.items()))
print(f"{'POOLED':24s}", " ".join(f"s{s}:{100*v[0]/max(1,v[1]):4.1f}%" for s, v in pool.items()))
