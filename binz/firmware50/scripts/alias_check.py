#!/usr/bin/env python3
"""ENV-76: is a near-5 A block real current or scan/commutation aliasing?

    python scripts/alias_check.py CAPTURE...

Reads a frozen `sag-capture` dump (SAGSNAP frozen=1, SAGROW rows v3). The trigger block is the last 100 rows (the
ring freezes on the block verdict, recorded before accumulation), the reference block the 100 before it.
Per-scan current, mA = (zero_block/100 - (pa+pb+pc)) * 100 * 5000 / ma_allow, so that the mean over a block equals the
firmware's block mA. Position in the sector: fitted from the step sequence (ENV-76 amendment), 8 bins.

  raw delta            = mean(trigger) - mean(reference)
  phase-uniform delta  = mean over bins of per-bin means, trigger minus reference (bins present in both)
Verdict: (A) aliasing if |uniform| <= 0.5 * raw; (R) real if uniform >= 0.5 * raw. Refuses unfrozen captures,
missing calibration, or fewer than 200 rows.
"""
import re, statistics as st, sys

BINS = 8


def load(path):
    t = open(path, errors="replace").read()
    snap = re.search(r"SAGSNAP .*frozen=(\d)", t)
    if not snap:
        return None, "no SAGSNAP"
    if snap.group(1) != "1":
        return None, "not frozen (no hold block >= 4750)"
    g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
    zero, allow, ci = g("zero_start"), g("ma_allow"), g("mean_ci_us")
    if None in (zero, allow, ci) or ci <= 0:
        return None, f"missing calibration zero={zero} allow={allow} ci={ci}"
    rows = []
    for line in t.splitlines():
        if line.startswith("SAGROW "):
            f = [int(x) for x in line.split()[1:]]
            if len(f) != 15:
                return None, f"SAGROW with {len(f)} fields, expected 15"
            at, at_fine, pwm, bus, pa, pb, pc, bm, vm, fb, fv, streak, step, duty, since = f
            ma = (zero / 100 - (pa + pb + pc)) * 100 * 5000 / allow
            rows.append(dict(ma=ma, bus=bus, step=step, since=since, pwm=pwm, at=at))
    if len(rows) < 200:
        return None, f"only {len(rows)} rows"
    # ENV-76 amendment: place each scan in its sector by fitting a steady sector clock to the step sequence.
    t = [0]
    for a, b in zip(rows, rows[1:]):
        t.append(t[-1] + ((b["at"] - a["at"]) & 0xFFFF))
    best = (-1, None, None)
    for k in range(-50, 51):
        T = ci * (1 + k / 1000)
        for j in range(64):
            t0 = -j * T / 64
            hits = sum(1 for r, tt in zip(rows, t) if int((tt - t0) // T) % 6 + 1 == r["step"])
            if hits > best[0]:
                best = (hits, T, t0)
    hits, T, t0 = best
    frac = hits / len(rows)
    if frac < 0.9:
        return None, f"step-clock fit explains only {frac:.0%} of steps"
    for r, tt in zip(rows, t):
        s_ = (tt - t0) / T
        r["bin"] = min(BINS - 1, int((s_ - int(s_ // 1)) * BINS))
    return (rows, T, g("worst_ma"), g("hold_ma"), frac), None


def per_bin(rows):
    d = {}
    for r in rows:
        d.setdefault(r["bin"], []).append(r["ma"])
    return {k: st.mean(v) for k, v in d.items()}, {k: len(v) for k, v in d.items()}


for p in sys.argv[1:]:
    res, why = load(p)
    name = p.replace("\\", "/").split("/")[-1]
    if res is None:
        print(f"{name}: NO VERDICT ({why})")
        continue
    rows, ci, worst, hold, fit = res
    trig, ref = rows[-100:], rows[-200:-100]
    raw = st.mean(r["ma"] for r in trig) - st.mean(r["ma"] for r in ref)
    bt, nt = per_bin(trig)
    br, nr = per_bin(ref)
    common = sorted(set(bt) & set(br))
    uni = st.mean(bt[k] for k in common) - st.mean(br[k] for k in common)
    bus = st.mean(r["bus"] for r in trig) - st.mean(r["bus"] for r in ref)
    verdict = "A (aliasing)" if abs(uni) <= 0.5 * raw else ("R (real)" if uni >= 0.5 * raw else "undecided")
    print(f"{name}: worst {worst} hold {hold} T {ci:.1f}us fit {fit:.0%} | trigger {st.mean(r['ma'] for r in trig):.0f} mA, "
          f"reference {st.mean(r['ma'] for r in ref):.0f} mA | raw d {raw:+.0f}, phase-uniform d {uni:+.0f} "
          f"({len(common)} bins) | bus_raw d {bus:+.1f} codes -> {verdict}")
    print("   bin counts trigger", [nt.get(k, 0) for k in range(BINS)], "reference", [nr.get(k, 0) for k in range(BINS)])
    print("   bin mA trigger   ", [round(bt.get(k, float('nan'))) for k in range(BINS)])
    print("   bin mA reference ", [round(br.get(k, float('nan'))) for k in range(BINS)])
