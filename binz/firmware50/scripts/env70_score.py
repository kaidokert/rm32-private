#!/usr/bin/env python3
"""ENV-70 (step 3): apply the predeclared attribution rule to the T/S/A captures.

    python scripts/env70_score.py

Per rung: E = mean(A) - mean(T), f = (mean(S) - mean(T)) / E on the worst block and on the zero-adjusted hold
(slope 0.021 mA/code on zero_start, pooled per rung as hold_covariate.py). Stopped runs (reason != 2) are listed and
excluded from the means; a side with fewer than 2 unstopped runs gives that rung no verdict. E <= 50 mA (worst) gives no
verdict. f >= 0.5 at both rungs -> (W); f <= 0.25 at both -> (R); else undecided. Refuses incomplete captures.
"""
import glob, os, re, statistics as st, sys

SLOPE = 0.021


def rows(rung):
    out = []
    for f in sorted(glob.glob(f"captures/2026-09-*/env70-{rung}-*_01.txt")):
        t = open(f, errors="replace").read()
        g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
        side = re.search(rf"env70-{rung}-\d+([TSA])_", f).group(1)
        if g("BEMFDONE reason") is None:
            sys.exit(f"REFUSED: incomplete capture {os.path.basename(f)}")
        hm = re.search(r"BEMFCURRENT.* hold_ma=(\d+)", t)
        out.append(dict(f=os.path.basename(f), side=side, reason=g("BEMFDONE reason"), worst=g("worst_ma"),
                        hold=int(hm.group(1)) if hm else None, zero=g("zero_start"), coast=g("coast_ehz"),
                        ceil=g("ceiling_tenths")))
    return out


verdicts = []
for rung in (725, 750):
    rs = rows(rung)
    for r in rs:
        print(rung, r)
    if len(rs) != 9:
        sys.exit(f"REFUSED: rung {rung} has {len(rs)} captures, expected 9")
    ok = [r for r in rs if r["reason"] == 2]
    zmean = st.mean(r["zero"] for r in ok)
    for r in ok:
        r["adj"] = r["hold"] - SLOPE * (r["zero"] - zmean)
    means = {}
    for s in "TSA":
        side = [r for r in ok if r["side"] == s]
        stops = [(r["f"], r["reason"]) for r in rs if r["side"] == s and r["reason"] != 2]
        if stops:
            print(f"  {rung} {s}: stops {stops}")
        if len(side) < 2:
            means[s] = None
            continue
        means[s] = dict(worst=st.mean(r["worst"] for r in side), adj=st.mean(r["adj"] for r in side),
                        coast=st.mean(r["coast"] for r in side), n=len(side))
        print(f"  {rung} {s}: n={len(side)} worst {means[s]['worst']:.0f} adj-hold {means[s]['adj']:.0f} coast {means[s]['coast']:.0f}")
    if any(means[s] is None for s in "TSA"):
        print(f"  {rung}: NO VERDICT (a side has < 2 unstopped runs)")
        verdicts.append(None)
        continue
    for k in ("worst", "adj"):
        E = means["A"][k] - means["T"][k]
        f = (means["S"][k] - means["T"][k]) / E if E else float("nan")
        print(f"  {rung} {k}: E = {E:+.0f} mA, f = {f:+.2f}")
    E = means["A"]["worst"] - means["T"]["worst"]
    if E <= 50:
        print(f"  {rung}: NO VERDICT (excess {E:+.0f} mA not reproduced)")
        verdicts.append(None)
    else:
        verdicts.append((means["S"]["worst"] - means["T"]["worst"]) / E)
if None in verdicts:
    print("STEP 3: NO VERDICT at", [r for r, v in zip((725, 750), verdicts) if v is None])
elif all(v >= 0.5 for v in verdicts):
    print("STEP 3: (W) the one-step-ahead wait -> step 5 justified")
elif all(v <= 0.25 for v in verdicts):
    print("STEP 3: (R) read timing / detection -> step 5 not justified by step 3")
else:
    print("STEP 3: UNDECIDED", [f"{v:+.2f}" for v in verdicts])
