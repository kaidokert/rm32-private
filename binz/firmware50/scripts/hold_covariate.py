#!/usr/bin/env python3
"""ENV-58/59: hold current with zero_start as a predeclared covariate (ENV-57 review).

    python scripts/hold_covariate.py GLOB_A GLOB_B

Adjusts each run's hold_ma to a common zero datum using the pooled within-image slope
0.021 mA/code (ENV-57), then compares arm means. Prints both raw and adjusted, and the verdict
against +/-3 %. Fails loudly on a capture without hold_ma or zero_start."""
import glob, re, statistics as st, sys
SLOPE = 0.021
def rows(pat):
    out = []
    for f in sorted(glob.glob(pat)):
        t = open(f).read()
        h = re.search(r"BEMFCURRENT.* hold_ma=(\d+)", t); z = re.search(r"zero_start=(\d+)", t)
        assert h and z, f"{f}: missing hold_ma/zero_start"
        out.append((f.replace("\\", "/").split("/")[-1], int(h.group(1)), int(z.group(1))))
    assert len(out) >= 3, f"{pat}: only {len(out)} captures"
    return out
A, B = rows(sys.argv[1]), rows(sys.argv[2])
z0 = st.mean([z for _, _, z in A + B])
adj = lambda r: r[1] - SLOPE * (r[2] - z0)
ra, rb = st.mean(r[1] for r in A), st.mean(r[1] for r in B)
aa, ab = st.mean(adj(r) for r in A), st.mean(adj(r) for r in B)
for n, h, z in A + B:
    print(f"  {n}: hold {h} zero_start {z} adjusted {h - SLOPE * (z - z0):.1f}")
d = (ab - aa) / aa * 100
print(f"raw: A {ra:.1f} B {rb:.1f} ({(rb - ra) / ra * 100:+.2f} %); adjusted: A {aa:.1f} B {ab:.1f} ({d:+.2f} %) -> "
      f"{'PASS' if abs(d) <= 3 else 'FAIL'} (bar +/-3 %)")
