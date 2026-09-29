#!/usr/bin/env python3
"""ENV-38b: judge the anchored low-rung holds on every cohort gate EXCEPT the previous-motor
oracle coast gate (Q60-3), plus the within-run self-referential identity the 525+ rungs use.
Refuses a rung with fewer than 3 complete captures. Loud on any other failure."""
import glob, os, pathlib, re, sys
sys.path.insert(0, os.path.dirname(__file__))
import cohort
ORACLE_RE = re.compile(r"^coast \d+ eHz outside 5% of \d+$")
ok = True
# Driven at 425 by the zero-step climb bug (plus() printed one "+" for n=0); excluded by name.
EXCLUDED = set()
SETS = [(R, f"env60-k{R}-") for R in (150, 200, 250, 275, 288, 300, 325, 338, 350)] + [(R, f"env60-r{R}-") for R in (375, 400, 425, 450, 475, 500)]
for R, pre in SETS:
    files = sorted(glob.glob(f"captures/2026-09-29/{pre}*_01.txt"))
    rows = []
    for f in files:
        if os.path.basename(f) in EXCLUDED:
            print(f"  (excluded, climb bug: {os.path.basename(f)})"); continue
        r = cohort.parse(pathlib.Path(f))
        if r is None or r.get("reason") is None:
            print(f"  (incomplete: {os.path.basename(f)})"); continue
        if r.get("duty") != R:
            print(f"  WRONG DUTY {os.path.basename(f)}: {r.get('duty')}"); ok = False; continue
        fails = cohort.run_gates(r)
        other = [x for x in fails if not ORACLE_RE.match(x)]
        oracle = [x for x in fails if ORACLE_RE.match(x)]
        selfref = cohort.self_ref_fails(r)
        rows.append((os.path.basename(f), r, other, oracle, selfref))
    for name, r, other, oracle, selfref in rows:
        print(f"{R} {name[:16]} coast={r['coast_ehz']} hold={r.get('hold_ma')} worst={r.get('worst_ma')} "
              f"reason={r['reason']} other={other or 'none'} oracle={'fail' if oracle else 'pass'} selfref={selfref or 'PASS'}")
    if len(rows) < 3:
        print(f"  {R}: only {len(rows)} complete captures -> INCOMPLETE"); ok = False; continue
    if any(o or s for _, _, o, _, s in rows):
        print(f"  {R}: FAIL"); ok = False
    else:
        print(f"  {R}: PASS on all non-oracle gates + self-ref 3/3; oracle {sum(1 for x in rows if x[3])}/3 fail")
print("LOW RUNGS, NON-ORACLE GATES + SELF-REF:", "PASS (oracle reported per rung; oracle failures = NOT PASSED by the ladder)" if ok else "NOT PASSED")
