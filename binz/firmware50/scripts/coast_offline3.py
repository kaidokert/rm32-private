"""Is the residual gap at 50% a window mismatch rather than a rate error?

The loop figure is a whole-hold average; the coast is a snapshot taken after
bridge-off. If the rotor slows across the hold, the two differ with no extra
commutations -- binz's point. The firmware already reports the hold's LAST
interval (`BEMFGATE ehz_from_ci_last`), which is contemporaneous with the
coast, so the comparison can be made like-for-like.
"""
import pathlib
import re
import statistics

BASE = pathlib.Path(r'E:\m\robot\esc\rm32\binz\firmware50\captures')


def last(text, key):
    ms = re.findall(rf'^{key} (.*)$', text, re.M)
    if not ms:
        return {}
    return {k: v for k, v in (t.split('=', 1) for t in ms[-1].split() if '=' in t)}


def coast_time(iv, first_us):
    h = iv[1:]
    t = first_us + iv[0]
    s = []
    for i in range(len(h) - 1):
        # E285: a real half-period paired with zero padding sums positive, so
        # this filter admitted it as a spurious half-length cycle. Require
        # BOTH halves present, which is the same rule as `speed.coast_fit`.
        per = h[i] + h[i + 1]
        if h[i] > 0 and h[i + 1] > 0:
            s.append((t + per / 2.0, per))
        t += h[i]
    n = len(s)
    if n < 3:
        return 0.0
    mt = sum(x[0] for x in s) / n
    mp = sum(x[1] for x in s) / n
    stt = sum((x[0] - mt) ** 2 for x in s)
    stp = sum((x[0] - mt) * (x[1] - mp) for x in s)
    if stt == 0:
        return 0.0
    at0 = mp + (stp / stt) * (-mt)
    return 1e6 / at0 if at0 > 0 else 0.0


rows = []
for f in sorted(BASE.glob('2026-09-2*/*.txt')):
    t = f.read_text(errors='replace')
    run, rate, gate, coast = last(t, 'BEMFRUN'), last(t, 'BEMFRATE'), last(t, 'BEMFGATE'), last(t, 'COASTTIMING')
    if not (run and rate and gate and coast):
        continue
    hold = int(rate.get('hold_ms', 0))
    acc = int(rate.get('hold_accepted', 0))
    iv = [int(x) for x in coast.get('iv_us', '').split(',') if x]
    last_ehz = int(gate.get('ehz_from_ci_last', 0))
    if hold < 9000 or acc == 0 or len(iv) < 5 or last_ehz == 0:
        continue
    ct = coast_time(iv, int(coast.get('first_us', 0)))
    if not ct:
        continue
    avg_ehz = 1e6 / (6.0 * (hold * 1000.0 / acc))
    rows.append(dict(duty=int(run.get('target_duty_tenths', 0)), file=f.name,
                     avg=avg_ehz, last=last_ehz, coast=ct))

print(f'{len(rows)} captures\n')
print('duty   n   hold-average   hold-LAST-interval   coast(t=0)  | avg vs coast   last vs coast')
for duty in sorted({r['duty'] for r in rows}):
    sub = [r for r in rows if r['duty'] == duty]
    if len(sub) < 3:
        continue
    a = statistics.mean(r['avg'] for r in sub)
    l = statistics.mean(r['last'] for r in sub)
    c = statistics.mean(r['coast'] for r in sub)
    print(f'{duty/10:5.1f} {len(sub):3d}   {a:12.1f}   {l:18.1f}   {c:10.1f}  | '
          f'{100*(a-c)/c:+12.2f}%  {100*(l-c)/c:+12.2f}%')

print('\nthe 50% runs, per capture:')
for r in rows:
    if r['duty'] == 500:
        print(f'  {r["file"]:<20} avg {r["avg"]:7.1f}  last {r["last"]:7.1f}  coast {r["coast"]:7.1f}  '
              f'avg-coast {100*(r["avg"]-r["coast"])/r["coast"]:+5.2f}%  last-coast {100*(r["last"]-r["coast"])/r["coast"]:+5.2f}%')
