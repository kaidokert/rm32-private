#!/usr/bin/env python3
"""ENV-28: what does the comparator do inside a late sector?

Reads an edge-capture dump (`CAP count reads wait average kind rising advance`; kind 0 accept,
1 too-early, 2 unstable, 3 rebase; reads: live comparator reads, oldest bit 0, count in bits
12..15). Groups offers into sectors (each ends at an accept; its `count` is the sector interval).
A sector is LATE if its interval > 1.25 x the mean of the 6 before. Prints, for late sectors
and a sample of normal ones, every offer as  t=<us since last accept>:<kind><reads>  where
reads print oldest first, 1 = comparator at the expected post-crossing level.
Refuses a dump without CAP rows."""
import sys, statistics as st
KIND = {0: "ACC", 1: "early", 2: "unst", 3: "rebase"}
def bits(reads, rising):
    n = (reads >> 12) & 0xF
    s = "".join("1" if (((reads >> i) & 1) == rising) else "0" for i in range(n))
    return s
for path in sys.argv[1:]:
    rows = [list(map(int, l.split()[1:])) for l in open(path) if l.startswith("CAP ")]
    assert rows, "REFUSED: no CAP rows"
    sectors, cur = [], []
    for r in rows:
        cur.append(r)
        if r[4] == 0:
            sectors.append(cur); cur = []
    iv = [s[-1][0] for s in sectors]
    late, normal = [], []
    for k in range(6, len(sectors)):
        ref = st.mean(iv[k-6:k])
        (late if iv[k] > 1.25 * ref else normal if abs(iv[k] - ref) < 0.1 * ref else []).append((k, ref))
    def offers_first_unst(k):
        s = sectors[k]
        return [o[0] for o in s if o[4] == 2]
    print(f"{path.split('/')[-1]}: {len(rows)} decisions, {len(sectors)} sectors, late {len(late)}, normal {len(normal)}")
    n_unst_norm = [sum(1 for o in sectors[k] if o[4] == 2) for k, _ in normal]
    n_unst_late = [sum(1 for o in sectors[k] if o[4] == 2) for k, _ in late]
    first_norm = [min((o[0] for o in sectors[k] if o[4] in (0, 2)), default=None) for k, _ in normal]
    first_late = [min((o[0] for o in sectors[k] if o[4] in (0, 2)), default=None) for k, _ in late]
    fn = [x for x in first_norm if x is not None]; fl = [x for x in first_late if x is not None]
    print(f"  unstable offers per sector: normal {st.mean(n_unst_norm):.2f}, late {st.mean(n_unst_late) if n_unst_late else 0:.2f}")
    if fn and fl:
        print(f"  first post-gate edge (us since last accept): normal median {st.median(fn)}, late median {st.median(fl)}")
    for k, ref in late[:8]:
        s = sectors[k]
        pol = "R" if s[-1][5] else "F"
        print(f"  LATE sector {k} ({pol}) interval {iv[k]} vs {ref:.0f}: " +
              "  ".join(f"t{o[0]}:{KIND[o[4]]}{bits(o[1], o[5])}" for o in s))
    for k, ref in normal[:4]:
        s = sectors[k]
        pol = "R" if s[-1][5] else "F"
        print(f"  normal {k} ({pol}) interval {iv[k]}: " + "  ".join(f"t{o[0]}:{KIND[o[4]]}{bits(o[1], o[5])}" for o in s))
