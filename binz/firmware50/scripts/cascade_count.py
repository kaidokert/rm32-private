#!/usr/bin/env python3
"""ENV-32: count cascade events in chain captures. Late = interval > 1.25 x the 6-accept
running mean; short = < 0.75 x. A CASCADE is a late accept followed within the next 2 accepts
by another late or a short. Prints late, short, cascades per capture. Refuses < 100 accepts."""
import sys, statistics as st
for path in sys.argv[1:]:
    A = [list(map(int, l.split()[1:])) for l in open(path) if l.startswith("CHAIN ") and l.split()[1] == "1"]
    assert len(A) >= 100, f"REFUSED {path}"
    iv = [((q[1] - p[1]) & 0xFFFF) for p, q in zip(A, A[1:])]
    cls = []
    for k in range(len(iv)):
        if k < 6: cls.append("n"); continue
        ref = st.mean(iv[k-6:k])
        cls.append("L" if iv[k] > 1.25 * ref else "S" if iv[k] < 0.75 * ref else "n")
    late = cls.count("L"); short = cls.count("S")
    casc = sum(1 for k, c in enumerate(cls) if c == "L" and any(x in "LS" for x in cls[k+1:k+3]))
    print(f"{path.split('/')[-1][:22]:22s} late {late:3d} short {short:3d} cascades {casc:3d}")
