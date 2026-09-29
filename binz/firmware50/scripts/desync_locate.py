#!/usr/bin/env python3
"""ENV-25: look for a commutation slip in a chain-capture dump whose rings were frozen at
the first AverageCurrent over-block (or at the run end, for a control).

Accept rows (kind 1) carry the crossing's fine stamp (TIM2, 125 ns, 16-bit wrap 8.192 ms)
and the sector (`step`). Per consecutive accept: interval = wrap-safe fine delta. A slip
candidate is an interval more than 25 % off the mean of the previous 6 accepted intervals,
or a sector sequence that does not advance by exactly one (mod 6). Prints the freeze cause
(from the run's own report) and every candidate in the ring, newest last, with its age
before the freeze.

Refuses: missing fine_hz, fewer than 100 accepts, any accept delta beyond the 16-bit span
(an unpairable gap is reported, never guessed)."""
import re, sys, statistics as st
path = sys.argv[1]
t = open(path).read()
hz = re.search(r"CHAINSNAP [^\n]*fine_hz=(\d+)", t)
assert hz, "REFUSED: no fine_hz"
tick_us = 1e6 / int(hz.group(1))
acc = []
for line in t.splitlines():
    if line.startswith("CHAIN "):
        f = line.split()
        kind = int(f[1])
        if kind == 1:
            acc.append((int(f[3]), int(f[7])))           # at_fine, step
assert len(acc) >= 100, f"REFUSED: only {len(acc)} accepts"
reason = int(re.search(r"BEMFDONE reason=(\d+)", t).group(1))
duty = int(re.search(r"target_duty_tenths=(\d+)", t).group(1))
ceil = int(re.search(r"ceiling_tenths=(\d+)", t).group(1))
cause = ("FOLDBACK (first over-block)" if ceil < duty else
         f"STOP reason {reason}" if reason != 2 else "normal window end (control/tail)")
iv = []
for (a, _), (b, _) in zip(acc, acc[1:]):
    d = (b - a) & 0xFFFF
    iv.append(d * tick_us)
span = sum(iv)
print(f"{path.split('/')[-1]}: duty {duty}, freeze: {cause}; {len(acc)} accepts over {span/1000:.1f} ms,"
      f" mean interval {st.mean(iv):.1f} us (sd {st.pstdev(iv):.1f})")
cands = []
age = 0.0
ages = [0.0]*len(iv)
for k in range(len(iv)-1, -1, -1):
    ages[k] = age; age += iv[k]
for k in range(6, len(iv)):
    ref = st.mean(iv[k-6:k])
    dev = (iv[k] - ref) / ref
    s0, s1 = acc[k][1], acc[k+1][1]
    seq_ok = (s1 - s0) % 6 in (1, 5)       # forward or reverse by one
    if abs(dev) > 0.25 or not seq_ok:
        cands.append((ages[k], iv[k], ref, dev, s0, s1, seq_ok))
print(f"  slip candidates (|dev|>25% or sector jump): {len(cands)}")
for a, x, r, d, s0, s1, ok in cands[-15:]:
    print(f"    {a/1000:6.2f} ms before freeze: interval {x:6.1f} us vs avg {r:5.1f} ({d:+.0%})  step {s0}->{s1}{'' if ok else '  SECTOR JUMP'}")
last10 = [c for c in cands if c[0] <= 10000]
print(f"  within the last 10 ms: {len(last10)}")
