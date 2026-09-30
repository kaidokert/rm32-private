#!/usr/bin/env python3
"""Goal B walk checker (ENV-63 rule, parameterised): python scripts/walk_check.py LABEL (e.g. env67).
isr_stats=0 images: spent/thin are compiled out and read 0, so the timing gate is late_arms == 0 alone; an
isr_stats=1 image keeps ENV-63's spent <= 11. A capture without the isr_stats key is refused (unknown image class).
Original: ENV-63 (A6 stage 2): apply the amended, predeclared rule mechanically to the walk captures (no hand assembly).
Per run: reason, ceiling, coast, hold, worst block, late_arms, spent_max_us, thin_count, ci_min, mdep_n.
Rule: every walk run late_arms == 0 and spent_max_us <= 11; 725: worst < 4750 in the 3 confirmatory holds,
no foldback. Prints PASS/FAIL per criterion. Refuses if a rung has fewer than 3 captures."""
import glob, os, re, sys
LABEL = sys.argv[1]
TOP = int(sys.argv[2]) if len(sys.argv) > 2 else 725  # the confirmatory top rung (worst < 4750, no foldback)
from collections import defaultdict
runs = defaultdict(list)
INCOMPLETE = []
for f in sorted(glob.glob(f"captures/2026-09-*/{LABEL}-r[0-9]*_01.txt")):
    t = open(f).read()
    g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
    hm = re.search(r"BEMFCURRENT.* hold_ma=(\d+)", t)
    rung = int(re.search(LABEL + r"-r(\d+)-", f).group(1))
    if rung < 525:
        continue
    if g("BEMFDONE reason") is None:
        # ENV-52 review: a capture with no BEMFDONE is a failure to report, not a skip.
        print(f"  INCOMPLETE CAPTURE (no BEMFDONE): {os.path.basename(f)}"); INCOMPLETE.append(os.path.basename(f)); continue
    runs[rung].append(dict(f=os.path.basename(f)[:14], reason=g("BEMFDONE reason"), ceil=g("ceiling_tenths"),
        coast=g("coast_ehz"), hold=int(hm.group(1)) if hm else None, worst=g("worst_ma"), late=g("late_arms"),
        spent=g("spent_max_us"), thin=g("thin_count"), cimin=g("ci_min_us"), mdep=g("mdep_n"), stats=g("isr_stats")))
ok = True
for rung in sorted(runs):
    rs = runs[rung]
    for r in rs:
        print(f"{rung} {r}")
    # ENV-51/52: a non-normal stop (reason != 2) is a failure -- checked BEFORE the run-count
    # test, because a latch usually ends a rung before its third run.
    stopped = [(r["f"], r["reason"]) for r in rs if r["reason"] != 2]
    if stopped: ok = False; print(f"  {rung}: STOP-REASON FAIL {stopped}")
    if len(rs) < 3:
        print(f"  {rung}: only {len(rs)} captures"); continue
    unk = [r["f"] for r in rs if r["stats"] is None]
    if unk: ok = False; print(f"  {rung}: REFUSED, no isr_stats key {unk}")
    bad = [r["f"] for r in rs if r["late"] != 0 or (r["stats"] == 1 and (r["spent"] or 99) > 11)]
    if bad: ok = False; print(f"  {rung}: TIMING FAIL {bad}")
    if rung == TOP:
        w = [r["worst"] for r in rs]; fb = [r for r in rs if r["ceil"] != TOP]
        print(f"  {TOP} confirmatory worst {w} -> {'PASS' if all(x < 4750 for x in w) else 'FAIL'}; foldbacks {len(fb)}")
        ok &= all(x < 4750 for x in w) and not fb
if INCOMPLETE: ok = False
need = list(range(525, TOP + 1, 25))
missing = [r for r in need if len(runs.get(r, [])) < 3]
if missing:
    print("RULE: INCOMPLETE (rungs lacking 3 completed captures:", missing, ")", "- so far", "no failure" if ok else "FAIL")
else:
    print(f"RULE (timing + {TOP} criteria):", "PASS" if ok else "FAIL")
