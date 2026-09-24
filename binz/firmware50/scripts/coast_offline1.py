"""Offline pass agreed with both reviewers, before any powered run.

1. `late_arms` and `blank_latched` across every capture -- the two reviews
   contradict each other on whether late arms ever occurred.
2. The loop's speed recomputed WITHOUT the firmware's whole-microsecond
   rounding: accepted events over the stamped hold, against the printed
   `ehz_from_sector`.
3. The coast speed at bridge-off estimated in TIME, anchored on the recorded
   first-transition delay (`first_us`), instead of against sample index with a
   fixed offset.
4. The resulting rate-identity ratio per rung, three ways.
"""
import pathlib
import re
import statistics

BASE = pathlib.Path(r'E:\m\robot\esc\rm32\binz\firmware50\captures')


def fields(text, key):
    m = re.search(rf'^{key} (.*)$', text, re.M)
    if not m:
        return {}
    out = {}
    for tok in m.group(1).split():
        if '=' in tok:
            k, v = tok.split('=', 1)
            out[k] = v
    return out


def last_fields(text, key):
    ms = re.findall(rf'^{key} (.*)$', text, re.M)
    if not ms:
        return {}
    out = {}
    for tok in ms[-1].split():
        if '=' in tok:
            k, v = tok.split('=', 1)
            out[k] = v
    return out


def pairs_with_time(iv, first_us, drop_first_half):
    """[(t_centre_us_from_bridge_off, period_us)] for consecutive half-period pairs."""
    h = iv[1:] if drop_first_half else iv
    base = first_us + (iv[0] if drop_first_half else 0)
    out = []
    t = base
    for i in range(len(h) - 1):
        per = h[i] + h[i + 1]
        if per <= 0:
            t += h[i]
            continue
        out.append((t + per / 2.0, per))
        t += h[i]
    return out


def fit_at_zero(samples):
    """Least squares period(t), read at t=0 (the bridge-off instant)."""
    n = len(samples)
    if n < 3:
        return 0.0
    mt = sum(t for t, _ in samples) / n
    mp = sum(p for _, p in samples) / n
    stt = sum((t - mt) ** 2 for t, _ in samples)
    stp = sum((t - mt) * (p - mp) for t, p in samples)
    if stt == 0:
        return 0.0
    slope = stp / stt
    at0 = mp + slope * (0 - mt)
    return at0


rows = []
for f in sorted(BASE.glob('2026-09-2*/*.txt')):
    t = f.read_text(errors='replace')
    run = fields(t, 'BEMFRUN')
    rate = last_fields(t, 'BEMFRATE')
    coast = last_fields(t, 'COASTTIMING')
    done = last_fields(t, 'BEMFDONE')
    rc = last_fields(t, 'BEMFRCOMP')
    drv = last_fields(t, 'BEMFDRIVEN')
    if not (run and rate and coast and done):
        continue
    hold_ms = int(rate.get('hold_ms', 0))
    acc = int(rate.get('hold_accepted', 0))
    if hold_ms < 9000 or acc == 0:
        continue
    iv = [int(x) for x in coast.get('iv_us', '').split(',') if x]
    if len(iv) < 5:
        continue
    duty = int(run.get('target_duty_tenths', 0))
    first_us = int(coast.get('first_us', 0))
    # 2: rotor rate from the accepted-event count over the stamped hold
    sector_us_true = hold_ms * 1000.0 / acc
    ehz_accepts = 1e6 / (6.0 * sector_us_true)
    ehz_printed = int(rate.get('ehz_from_sector', 0))
    # 3: coast at bridge-off, in time, both variants
    p_all = fit_at_zero(pairs_with_time(iv, first_us, False))
    p_drop = fit_at_zero(pairs_with_time(iv, first_us, True))
    # and the estimator in use (index-based, first half-period dropped)
    ps = [iv[i] + iv[i + 1] for i in range(1, len(iv) - 1)]
    n = len(ps)
    mx = (n - 1) / 2
    my = sum(ps) / n
    sxx = sum((x - mx) ** 2 for x in range(n))
    sxy = sum((x - mx) * (y - my) for x, y in enumerate(ps))
    p_index = my + (sxy / sxx) * (-0.5 - mx) if sxx else my
    rows.append(dict(
        file=f.name, duty=duty, hold_ms=hold_ms,
        reason=int(done.get('reason', 0)),
        late_arms=int(rc.get('late_arms', -1)),
        blank_latched=int(drv.get('blank_latched', -1)),
        spent=int(rc.get('spent_max_us', 0)),
        com_late=int(drv.get('com_late_max_us', 0)),
        zc_per_s=int(rate.get('zc_per_s', 0)),
        ehz_accepts=ehz_accepts, ehz_printed=ehz_printed,
        coast_index=1e6 / p_index if p_index > 0 else 0,
        coast_time_all=1e6 / p_all if p_all > 0 else 0,
        coast_time_drop=1e6 / p_drop if p_drop > 0 else 0,
        first_us=first_us,
    ))

print(f'{len(rows)} captures with a real dwell\n')

print('=== 1. late_arms / blank_latched, every non-zero ===')
bad = [r for r in rows if r['late_arms'] > 0 or r['blank_latched'] > 0]
if not bad:
    print('  none')
for r in bad:
    print(f"  {r['file']:<24} duty {r['duty']:>3}  reason {r['reason']:>2}  "
          f"late_arms {r['late_arms']}  blank_latched {r['blank_latched']}  spent {r['spent']}  com_late {r['com_late']}")
print(f'  -> {len(bad)} of {len(rows)} captures have a non-zero counter')

print('\n=== 2/3/4. per rung: printed vs unrounded vs coast, and the ratio three ways ===')
print('duty   n  printed  unrounded  d%    coast_idx coast_t_all coast_t_drop | ratio_idx ratio_t_drop')
for duty in sorted({r['duty'] for r in rows}):
    sub = [r for r in rows if r['duty'] == duty]
    if len(sub) < 3:
        continue
    pr = statistics.mean(r['ehz_printed'] for r in sub)
    un = statistics.mean(r['ehz_accepts'] for r in sub)
    ci = statistics.mean(r['coast_index'] for r in sub)
    ca = statistics.mean(r['coast_time_all'] for r in sub)
    cd = statistics.mean(r['coast_time_drop'] for r in sub)
    ri = statistics.mean(1000 * r['zc_per_s'] / (6 * r['coast_index']) for r in sub if r['coast_index'])
    rd = statistics.mean(1000 * r['zc_per_s'] / (6 * r['coast_time_drop']) for r in sub if r['coast_time_drop'])
    print(f'{duty/10:5.1f} {len(sub):3d}  {pr:7.1f}  {un:9.1f}  {100*(pr-un)/un:+5.2f}  '
          f'{ci:9.1f} {ca:11.1f} {cd:12.1f} | {ri:9.1f} {rd:12.1f}')

print('\n=== first-transition delay, which the index-based estimator ignores ===')
for duty in sorted({r['duty'] for r in rows}):
    sub = [r['first_us'] for r in rows if r['duty'] == duty]
    if len(sub) >= 3:
        print(f'  {duty/10:5.1f}%  first_us min {min(sub):5d}  median {int(statistics.median(sub)):5d}  max {max(sub):5d}')
