#!/usr/bin/env python3
"""ENV-50: per-step late counts against each step's OWN mean interval (the ENV-49 review's correction:
`late_by_step.py` compares to the capture mean, so a sector that is long by construction is flagged first).

    python scripts/late_own_mean.py CAPTURE...   (late = accept interval > 1.25 x that step's own mean)

Prints per capture: counts by step and total. Refuses a capture with fewer than 30 intervals in any step."""
import statistics as st, sys
for p in sys.argv[1:]:
    acc = []
    for l in open(p, errors="replace"):
        if l.startswith("CHAIN "):
            f = [int(x) for x in l.split()[1:9]]
            if f[0] == 1:
                acc.append((f[1], f[6]))
    iv = {s: [] for s in range(1, 7)}
    for (a, _), (b, s) in zip(acc, acc[1:]):
        d = (b - a) & 0xFFFF
        if d < 400:
            iv[s].append(d)
    for s, v in iv.items():
        assert len(v) >= 30, f"{p}: step {s} has only {len(v)} intervals"
    r = {s: sum(1 for x in v if x > 1.25 * st.mean(v)) for s, v in iv.items()}
    print(f"{p}: " + " ".join(f"s{s}:{r[s]}" for s in range(1, 7)) + f"  total {sum(r.values())}"
          f"  mean {st.mean([x for v in iv.values() for x in v]):.1f}")
