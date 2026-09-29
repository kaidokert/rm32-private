#!/usr/bin/env python3
"""ENV-25: is a late-accepted crossing masked by the post-commutation blanking window?

For every accept (kind 1) find the latest service row (kind 2) whose purpose is a blank
end (2 = reverse-blank end, 3 = blanking-floor arm) and the latest commutation (purpose 1)
before it, on the coarse microsecond clock (`at_us`, 16-bit, 65.5 ms wrap -- the ~35 ms
rings are unambiguous). Reports, for normal accepts vs accepts ending a LONG interval
(> +25 % over the 6-accept running mean):
  gap_blank = accept - last blank end   (small => accepted as soon as listening resumed)
  gap_com   = accept - last commutation
Refuses captures without both rings."""
import sys, statistics as st
def rows(path):
    A, S = [], []
    for l in open(path):
        if l.startswith("CHAIN "):
            f = list(map(int, l.split()[1:]))
            kind, at_us, at_fine, x_fine, y_us, z_fine, step, flag = f
            (A if kind == 1 else S).append((at_us, flag & 0x7F, step, at_fine))
    return A, S
def unwrap(seq):
    out, base, prev = [], 0, None
    for v in seq:
        if prev is not None and v < prev - 30000: base += 65536
        out.append(v + base); prev = v
    return out
for path in sys.argv[1:]:
    A, S = rows(path)
    assert len(A) > 100 and len(S) > 100, "REFUSED: rings missing"
    # put both on one unwrapped axis: unwrap the merged, time-sorted sequence by coarse us
    ta = unwrap([a[0] for a in A]); ts = unwrap([s[0] for s in S])
    # align origins: both rings end at the same freeze, so align their last stamps' raw values
    # by shifting the service axis so its raw last value maps consistently
    off = (ta[-1] - A[-1][0]) - (ts[-1] - S[-1][0])
    ts = [x + off for x in ts]
    iv = [(b - a) for a, b in zip(ta, ta[1:])]
    blanks = [(t, p) for t, (_, p, _, _) in zip(ts, S) if p in (2, 3)]
    coms = [t for t, (_, p, _, _) in zip(ts, S) if p == 1]
    res = {"normal": [], "long": []}
    for k in range(7, len(ta)):
        ref = st.mean(iv[k-7:k-1]); x = iv[k-1]
        kind = "long" if x > 1.25 * ref else "normal" if abs(x - ref) <= 0.1 * ref else None
        if kind is None: continue
        t = ta[k]
        b = [bt for bt, _ in blanks if bt <= t]; c = [ct for ct in coms if ct <= t]
        if not b or not c: continue
        res[kind].append((t - b[-1], t - c[-1], A[k][2]))
    print(path.split("/")[-1])
    for kind in ("normal", "long"):
        v = res[kind]
        if not v: print(f"  {kind:6s}: none"); continue
        gb = [a for a, _, _ in v]; gc = [b for _, b, _ in v]
        near = sum(1 for g in gb if g <= 3)
        print(f"  {kind:6s} n={len(v):4d}  accept-after-blank-end median {st.median(gb):5.1f} us"
              f" (<=3 us: {near}/{len(v)})   accept-after-commutation median {st.median(gc):5.1f} us")
    if res["long"]:
        print("   long accepts: (after blank end, after commutation, step):", res["long"][:12])
