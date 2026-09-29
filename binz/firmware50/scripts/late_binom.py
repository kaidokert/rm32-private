#!/usr/bin/env python3
"""ENV-61/62: one-sided conditional binomial test of own-mean late counts, A vs T (same session).

    python scripts/late_binom.py "GLOB_A" "GLOB_T"

Uses late_own_mean's definition (interval > 1.25 x that step's own mean). Tests all steps and step 3:
A of (A + T) against p = 0.5 (equal capture counts required); PASS if p-value >= 0.05 (A not significantly
greater). Refuses unequal capture counts."""
import glob, math, statistics as st, sys
def counts(p):
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
    return {s: sum(1 for x in v if x > 1.25 * st.mean(v)) for s, v in iv.items()}
def pval(k, n):  # P(X >= k), X ~ Bin(n, 0.5)
    return sum(math.comb(n, i) for i in range(k, n + 1)) / 2 ** n
A = [counts(p) for p in sorted(glob.glob(sys.argv[1]))]; T = [counts(p) for p in sorted(glob.glob(sys.argv[2]))]
assert len(A) == len(T) and A, f"need equal, non-zero capture counts: {len(A)} vs {len(T)}"
for label, f in (("all steps", lambda c: sum(c.values())), ("step 3", lambda c: c[3])):
    a, t = sum(f(c) for c in A), sum(f(c) for c in T)
    p = pval(a, a + t)
    print(f"{label}: A {a} vs T {t} (n={len(A)} captures each); one-sided p = {p:.3f} -> {'PASS' if p >= 0.05 else 'FAIL'}")
