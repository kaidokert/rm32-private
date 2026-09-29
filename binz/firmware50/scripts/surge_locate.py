#!/usr/bin/env python3
"""ENV-22: locate the 72.5 % current surge in a sag-capture ring frozen on the first
AverageCurrent over-block. Per scan: I = (zero/scan - (pa+pb+pc)) * 4000 / (RAW_LIMIT/100)
mA, zero from the capture's bridge-off zero_start. Prints the per-scan current in 16-scan
bins over time (oldest first; the ring ends at the freeze), and the mean by sector.
Refuses a capture that did not freeze or lacks rows (fail loudly)."""
import re, sys, statistics as st
RAW_LIMIT = 31_857
args = [a for a in sys.argv[1:] if not a.startswith("--")]
control = "--control" in sys.argv   # unfrozen ring: the run's final ~26 ms, a baseline
path = args[0]
t = open(path).read()
snap = re.search(r"SAGSNAP [^\n]*frozen=(\d)", t)
assert snap, "no SAGSNAP header"
# `frozen` is NOT evidence of an over-block: the ring also freezes at every stop,
# including the normal window end (ENV-23). Classify the freeze from the run itself.
reason = int(re.search(r"BEMFDONE reason=(\d+)", t).group(1))
duty = int(re.search(r"target_duty_tenths=(\d+)", t).group(1))
ceil = int(re.search(r"ceiling_tenths=(\d+)", t).group(1))
cause = ("foldback (first AverageCurrent over-block)" if ceil < duty
         else f"stop reason {reason}" if reason != 2 else "normal window end (a tail, not an event)")
print(f"freeze cause: {cause}  [reason={reason} duty={duty} ceiling={ceil}]")
gate = "--gate" in sys.argv   # ENV-42: ring frozen by the diagnostic `freeze-gate` (first hold block >= 4750 mA)
if gate:
    judged = int(re.search(r"SAGSNAP [^\n]*judged=(\d+)", t).group(1))
    ticks = int(re.search(r"BEMFGUARD [^\n]*ticks=(\d+)", t).group(1))
    early = ticks - judged
    print(f"gate freeze: judged={judged} ticks={ticks} -> froze ~{early*0.101/1000:.1f} s before the run's last judgement")
    assert early > 20_000, "the ring was NOT frozen by the gate (froze at the run's end): refusing"
elif control:
    assert ceil == duty and reason == 2, "--control needs a run with no foldback and a normal end"
else:
    assert ceil < duty or reason != 2, "no event froze this ring: it is the run's tail (use --control)"
z = int(re.search(r"zero_start=(\d+)", t).group(1)) / 100.0   # per scan, sum of 3 phases
rows = [list(map(int, l.split()[1:])) for l in t.splitlines() if l.startswith("SAGROW ")]
assert len(rows) >= 200, f"only {len(rows)} rows"
ma_per = 4000 / (RAW_LIMIT / 100)
cur = [(z - (r[4] + r[5] + r[6])) * ma_per for r in rows]
steps = [r[12] for r in rows]
szc = [r[14] for r in rows]
print(f"{path}: {len(rows)} scans, ~{len(rows)*0.101:.1f} ms, zero/scan {z:.1f}, {ma_per:.2f} mA/code")
print("bins of 16 scans (~1.6 ms), oldest first: mean mA [min..max]")
for i in range(0, len(cur), 16):
    b = cur[i:i+16]
    print(f"  {i:3d}-{i+15:3d}: {st.mean(b):6.0f}  [{min(b):6.0f} .. {max(b):6.0f}]")
first, last = cur[:128], cur[128:]
print(f"first half mean {st.mean(first):.0f} mA, last half (into the freeze) {st.mean(last):.0f} mA")
print("by sector (all 256 / last 100 scans): mean mA, n")
for s in sorted(set(steps)):
    a = [c for c, k in zip(cur, steps) if k == s]
    l = [c for c, k in zip(cur[-100:], steps[-100:]) if k == s]
    print(f"  step {s}: {st.mean(a):6.0f} (n={len(a):3d})   last100 {st.mean(l) if l else float('nan'):6.0f} (n={len(l)})")
