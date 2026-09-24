"""Which coast estimator is least noisy, judged on every stored capture.

The rate-identity gate compares accepted crossings per second against
6 x the coast speed. Its noise comes from the coast estimate: the rotor
decelerates across the eight half-periods the firmware reports, so an average
of them under-reads the speed at bridge-off, and by a run-dependent amount.

Estimators compared, all from the same `COASTTIMING iv_us` list:
  median_pair : the fixture's own (cohort.coast_ehz) -- median of consecutive
                pair sums.
  first_pair  : the first pair only (what E113 rejected as transient-prone).
  fit_pairs   : least squares through the pair sums against pair index,
                extrapolated back to the bridge-off instant (index -0.5).
  fit_drop1   : the same, with the first half-period dropped (E113's concern).
"""
import pathlib
import re
import statistics

BASE = pathlib.Path(r'E:\m\robot\esc\rm32\binz\firmware50\captures')


def pairs(iv):
    return [iv[i] + iv[i + 1] for i in range(len(iv) - 1) if iv[i] + iv[i + 1] > 0]


def fit_back(ps, back=0.5):
    n = len(ps)
    if n < 3:
        return 0
    xs = list(range(n))
    mx = sum(xs) / n
    my = sum(ps) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ps))
    if sxx == 0:
        return 0
    slope = sxy / sxx
    at = my + slope * (-back - mx)
    return round(1e6 / at) if at > 0 else 0


rows = []
for f in sorted(BASE.glob('2026-09-2*/*.txt')):
    t = f.read_text(errors='replace')
    m = re.search(r'iv_us=([\d,]+)', t)
    r = re.search(r'BEMFRATE .*?zc_per_s=(\d+)', t)
    d = re.search(r'target_duty_tenths=(\d+)', t)
    hold = re.search(r'BEMFRATE .*?hold_ms=(\d+)', t)
    if not (m and r and d and hold) or int(hold.group(1)) < 9000:
        continue
    iv = [int(x) for x in m.group(1).split(',')]
    ps = pairs(iv)
    if len(ps) < 4:
        continue
    zc = int(r.group(1))
    est = {
        'median_pair': round(1e6 / statistics.median(ps)),
        'first_pair': round(1e6 / ps[0]),
        'fit_pairs': fit_back(ps),
        'fit_drop1': fit_back(pairs(iv[1:])),
    }
    rows.append((f.name, int(d.group(1)), zc, est))

print(f'{len(rows)} captures with a real dwell\n')
for name in ('median_pair', 'first_pair', 'fit_pairs', 'fit_drop1'):
    ratios = [1000 * zc / (6 * e[name]) for _, _, zc, e in rows if e[name]]
    out = [x for x in ratios if not 990 <= x <= 1010]
    print(f'{name:<12} mean {statistics.mean(ratios):7.1f}  sd {statistics.pstdev(ratios):5.2f}  '
          f'min {min(ratios):6.1f}  max {max(ratios):6.1f}  outside 1%: {len(out)}/{len(ratios)}')

print('\nby duty, fixture estimator vs the best fit:')
duties = sorted({d for _, d, _, _ in rows})
for d in duties:
    sub = [(zc, e) for _, dd, zc, e in rows if dd == d]
    if len(sub) < 3:
        continue
    a = [1000 * zc / (6 * e['median_pair']) for zc, e in sub if e['median_pair']]
    b = [1000 * zc / (6 * e['fit_drop1']) for zc, e in sub if e['fit_drop1']]
    print(f'  {d/10:5.1f}%  n={len(a):3d}  median_pair {statistics.mean(a):7.1f} +-{statistics.pstdev(a):4.2f}   '
          f'fit_drop1 {statistics.mean(b):7.1f} +-{statistics.pstdev(b):4.2f}')
