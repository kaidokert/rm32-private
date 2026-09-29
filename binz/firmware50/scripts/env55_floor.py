#!/usr/bin/env python3
"""ENV-55: the blanking-floor decision and accept attribution, from chain-origin captures.

    python scripts/env55_floor.py CAPTURE...

Accept rows (kind 1): at_us = crossing (coarse µs), y_us = wait, flag bit 6 = revisit-originated (chain-origin).
Service rows (kind 2): at_fine = COM entry (coarse µs -- see edge_probe.py), flag & 0x7f = purpose (1 commutate,
3 floor end). Each accept is paired with its phase-1 service (first after the crossing within 2 x wait + 10 µs).

Per commutation:
  since     = COM phase-1 entry - crossing (µs)
  blanking  = avg / 2, avg = AM32 blend (avg + (last + this) / 2) / 2 of accept intervals, updated WITH this crossing
  hold      = blanking - since      (the firmware arms the floor iff hold >= 16, BLANK_ARM_MIN_US)
  floored   = a purpose-3 service row follows this phase-1 row before the next phase-1 row

The reconstruction is checked against the observed floors (agreement %); if it is below 90 % the capture is refused,
because then the reconstructed hold is not the firmware's.
Also: revisit-originated share of accepts, and split sectors (interval < 0.6 x avg).
"""
import statistics as st, sys

THRESH = 16


def analyse(path):
    acc, svc = [], []
    for line in open(path, errors="replace"):
        if line.startswith("CHAIN "):
            f = [int(x) for x in line.split()[1:9]]
            kind, at_us, at_fine, _x, y_us, _z, step, flag = f
            if kind == 1:
                acc.append((at_us, y_us, step, bool(flag & 0x40)))
            elif kind == 2:
                svc.append((at_fine, flag & 0x7F))
    assert acc and svc, f"{path}: no rows"
    # Running AM32 blend over accept intervals.
    avg = None
    last = None
    rows = []
    for (c0, *_), (c1, wait, step, rev) in zip(acc, acc[1:]):
        this = (c1 - c0) & 0xFFFF
        if this > 1000:
            avg = None
            last = None
            continue
        if avg is None:
            avg, last = this, this
        split = this < 0.6 * avg
        avg = (avg + (last + this) / 2) / 2
        last = this
        rows.append((c1, wait, step, rev, avg, split))
    lo, hi = svc[0][0], svc[-1][0]
    span = (hi - lo) & 0xFFFF
    out = []
    for c, wait, step, rev, a, split in rows:
        if ((c - lo) & 0xFFFF) > span:
            continue
        # phase-1 entry following the crossing
        idx = None
        for i, (e, p) in enumerate(svc):
            d = (e - c) & 0xFFFF
            if p == 1 and d <= 2 * wait + 10:
                idx = i
                break
        if idx is None:
            continue
        since = (svc[idx][0] - c) & 0xFFFF
        floored = idx + 1 < len(svc) and svc[idx + 1][1] == 3
        hold = a / 2 - since
        out.append(dict(since=since, blank=a / 2, hold=hold, floored=floored, rev=rev, split=split, step=step))
    assert len(out) > 100, f"{path}: only {len(out)} paired commutations"
    agree = sum(1 for o in out if (o["hold"] >= THRESH) == o["floored"]) / len(out)
    return out, agree, acc


def main():
    for p in sys.argv[1:]:
        out, agree, acc = analyse(p)
        name = p.replace("\\", "/").split("/")[-1]
        if agree < 0.9:
            print(f"{name}: REFUSED — reconstruction agrees with observed floors only {agree:.0%}")
            continue
        fl = sum(o["floored"] for o in out) / len(out)
        sin = [o["since"] for o in out]
        bl = [o["blank"] for o in out]
        ho = [o["hold"] for o in out]
        near = sum(1 for h in ho if abs(h - THRESH) <= 1) / len(ho)
        rev = sum(1 for a in acc if a[3]) / len(acc)
        split = sum(o["split"] for o in out) / len(out)
        print(f"{name}: n={len(out)} agree={agree:.0%} floor_rate={fl:.1%} since p50={st.median(sin)} "
              f"[{min(sin)}..{max(sin)}] blank p50={st.median(bl):.1f} hold p50={st.median(ho):.1f} "
              f"within±1us={near:.0%} revisit={rev:.1%} split={split:.1%}")


if __name__ == "__main__":
    main()
