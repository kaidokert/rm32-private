#!/usr/bin/env python3
"""ENV-46 edge probe: does each commutation fire at crossing + requested wait?

    python scripts/edge_probe.py CHAIN_CAPTURE...

Accept rows (kind 1) carry the crossing's coarse stamp `at_us` (TIM17, 1 MHz) and the requested wait
`y_us`. Phase-1 service rows (kind 2, purpose 1) carry the handler's entry stamp in the column chain.py
calls `at_fine` -- but **COM logs `now_raw` there, which is the same coarse 1 MHz clock**, not TIM2
(ENV-46; the service row's `at_us` is the *scheduled* stamp, read after a blanking-floor re-arm has
already overwritten it with the floor's schedule). So both are paired on the coarse clock, which wraps
every 65.536 ms against a ring span of ~27 ms at 37.5 %.

For every accept, the first phase-1 service whose entry follows the crossing within 2x the wait is its
commutation; residual = entry - (crossing + wait), µs. A commutation that fired at the accept instead
of after the wait (the rm32 UG-race signature) shows as residual ~ -wait.
Refuses a capture where fewer than 90 % of the accepts inside the service ring's span find a service.
"""
import statistics as st, sys

for path in sys.argv[1:]:
    acc, svc = [], []
    for line in open(path, errors="replace"):
        if line.startswith("CHAIN "):
            f = [int(x) for x in line.split()[1:9]]
            kind, at_us, at_fine, _x, y_us, _z, step, flag = f
            if kind == 1:
                acc.append((at_us, y_us, step))
            elif kind == 2 and (flag & 0x7F) == 1:
                svc.append((at_fine, step))
    assert acc and svc, f"{path}: missing accept or phase-1 service rows"
    lo, hi = svc[0][0], svc[-1][0]
    span = (hi - lo) & 0xFFFF
    inside = [a for a in acc if ((a[0] - lo) & 0xFFFF) <= span]
    res, steps_ok = [], 0
    for c, wait, step in inside:
        best = None
        for e, s in svc:
            d = (e - c) & 0xFFFF
            if d <= 2 * wait + 10 and (best is None or d < best[0]):
                best = (d, s)
        if best:
            res.append(best[0] - wait)
            steps_ok += best[1] == (1 if step == 6 else step + 1)
    assert len(res) >= 0.9 * len(inside), f"{path}: only {len(res)}/{len(inside)} accepts matched"
    q = st.quantiles(res, n=100)
    print(f"{path}: {len(res)}/{len(inside)} accepts matched (next step in {steps_ok}); "
          f"entry - (crossing + wait) us: min={min(res)} p1={q[0]:.1f} p50={st.median(res):.1f} "
          f"p99={q[98]:.1f} max={max(res)}")
