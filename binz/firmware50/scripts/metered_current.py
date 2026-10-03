"""Project the 60% draw from the METERED clamp bracket, not from the proxy.

E234 escalated "healthy 60% projects to ~2.7 A mean and ~3.2 A worst block
against a 3 A clamp, so 60% may be unqualifiable" -- by reading `hold_ma`, an
uncalibrated signed three-shunt residual anchored on one operator-metered
point, as amps. E230's own disposition had already rejected that as false
precision, and the escalation did it anyway.

The clamp setting *is* a meter, and both sides of the CV/CC transition at the
50% rung are already in the corpus:

  * at a 1.75 A clamp the run is HEALTHY -- the supply never left CV;
  * at a 1.6 A clamp the run is CC -- operator-confirmed, with `hold_ma`
    pinned and the bus folded.

So the true DC mean at 500 tenths is bracketed by those two settings, and the
proxy's scale error is measurable against it. The CV/CC discriminator itself is
`filt_bus/ref_bus`: a folded rail is the signature, not a current reading.

Also corrected here: a "worst block" is a 10.1 ms mean that the PSU's output
capacitors supply. Constant-current limiting is a *mean*-against-clamp
phenomenon, so a worst-block figure is not a CC criterion at all.
"""
import glob
import math
import os
import re
import statistics

CLAMP_A = 3.0


def runs():
    """(duty, hold_ma, worst_ma, droop_permille, name) per BEMFCURRENT block."""
    out = []
    for f in sorted(glob.glob('captures/**/*.txt', recursive=True)):
        t = open(f, encoding='utf-8', errors='replace').read()
        if 'warmup' in os.path.basename(f):
            continue
        # A run perturbed by an injection is not a healthy sample of its rung:
        # the sag positive control steps duty to INJECT_SAG_DUTY_TENTHS
        # mid-run, which is how a 475 capture came to read a 500 interval and
        # got pooled into "the interval has flattened".
        inj = re.search(r'\binject=(\d+)', t)
        provoked = (inj and inj.group(1) != '0') or 'provoked=1' in t
        # **Anchored on its line.** `reason` is emitted by THREE lines --
        # BEMFDONE, BEMFCOAST and BEMFGUARD -- and a flat `\breason=` matched
        # whichever came first in the file. That was BEMFDONE only by emission
        # order, not by anything guaranteed: reorder the report and this reader
        # silently starts classifying runs by the coast or guard reason instead.
        # Eight key names are ambiguous this way; see `cohort.ambiguous_keys`.
        rs = re.search(r'^BEMFDONE .*?\breason=(\d+)', t, re.M)
        sag = re.search(r'BEMFSAG ref_bus=(\d+) ref_vref=(\d+) filt_bus=(\d+) filt_vref=(\d+)', t)
        droop = None
        if sag:
            rb, rv, fb, fv = (int(x) for x in sag.groups())
            if rb and fv:
                droop = fb * rv * 1000 / (rb * fv)
        for m in re.finditer(r'BEMFCURRENT ([^\n]*)', t):
            d = dict(re.findall(r'(\w+)=(-?\d+)', m.group(1)))
            if 'hold_ma' not in d or 'duty_tenths' not in d:
                continue
            out.append(dict(duty=int(d['duty_tenths']), hold=int(d['hold_ma']),
                            worst=int(d.get('worst_ma', 0)), droop=droop,
                            provoked=bool(provoked),
                            reason=int(rs.group(1)) if rs else -1,
                            name=os.path.basename(f)))
    return out


def main():
    r = runs()
    print(f'BEMFCURRENT blocks parsed: {len(r)}')

    # The CV/CC split is the folded rail, not a current number.
    cc = [x for x in r if x['droop'] is not None and x['droop'] < 970
          and not x['provoked']]
    cv = [x for x in r if x['droop'] is not None and x['droop'] >= 975
          and not x['provoked']]
    print(f'\nCV/CC split on filt_bus/ref_bus (>=975 permille = CV, <970 = CC):')
    print(f'  CV blocks {len(cv)};  CC blocks {len(cc)}')
    for lbl, g in (('CC', cc), ('CV at 500', [x for x in cv if x['duty'] == 500])):
        if g:
            print(f'  {lbl}: droop {min(x["droop"] for x in g):.0f}-'
                  f'{max(x["droop"] for x in g):.0f} permille, '
                  f'hold_ma {min(x["hold"] for x in g)}-{max(x["hold"] for x in g)}')
            print(f'        captures: {sorted({x["name"] for x in g})[:6]}')

    print('\n=== the proxy against the metered bracket, at 500 tenths')
    h500 = [x['hold'] for x in cv if x['duty'] == 500]
    if not h500:
        raise SystemExit('no CV 500 blocks found')
    proxy = statistics.median(h500)
    print(f'  proxy hold_ma median {proxy:.0f} mA (n={len(h500)})')
    print(f'  metered bracket: CV at a 1.75 A clamp, CC at a 1.6 A clamp')
    print(f'  => true mean is in [1600, 1750] mA; proxy over-reads by '
          f'{100 * (proxy / 1750 - 1):.1f}% to {100 * (proxy / 1600 - 1):.1f}%')

    print('\n=== the exponent, healthy CV blocks only, per rung')
    per = {}
    for x in cv:
        per.setdefault(x['duty'], []).append(x['hold'])
    for d in sorted(per):
        v = per[d]
        print(f'  duty {d:4d}  n={len(v):3d}  median {statistics.median(v):7.1f} mA')
    ks = []
    duties = sorted(per)
    for i in range(len(duties) - 1):
        a, b = duties[i], duties[i + 1]
        ia, ib = statistics.median(per[a]), statistics.median(per[b])
        k = math.log(ib / ia) / math.log(b / a)
        ks.append(k)
        print(f'  {a}->{b}: k = {k:.3f}')
    klo, khi = min(ks), max(ks)

    print(f'\n=== projecting 600 tenths from the METERED bracket, k in '
          f'[{klo:.3f}, {khi:.3f}]')
    for k in (klo, khi):
        for anchor in (1600, 1750):
            i600 = anchor * (600 / 500) ** k
            print(f'  anchor {anchor} mA, k={k:.3f}  ->  I(600) = {i600:.0f} mA '
                  f'= {100 * i600 / (CLAMP_A * 1000):.0f}% of a {CLAMP_A:.0f} A clamp')
    lo = 1600 * (600 / 500) ** klo
    hi = 1750 * (600 / 500) ** khi
    print(f'\n  range: {lo:.0f}-{hi:.0f} mA against {CLAMP_A * 1000:.0f} mA '
          f'=> {100 * lo / (CLAMP_A * 1000):.0f}-{100 * hi / (CLAMP_A * 1000):.0f}%'
          f' of the clamp')
    print(f'  verdict: {"INSIDE the clamp (CV) with headroom" if hi < CLAMP_A * 1000 else "AT OR OVER the clamp"}')

    print('\n=== for contrast, the proxy-anchored projection E234 escalated')
    for k in (klo, khi):
        print(f'  proxy anchor {proxy:.0f} mA, k={k:.3f}  ->  '
              f'{proxy * (600 / 500) ** k:.0f} mA')
    print('  -- which is the same arithmetic on an anchor that reads high.')

    print('\n=== worst/hold ratio, healthy CV, per rung (E235 assumed 1.19 flat)')
    for d in sorted(per):
        # `hold` of 0 is a run that never reached its hold window (E329: a
        # refuted `wide-blank` image stopped on FastBusSag during the ramp), and
        # dividing by it crashed this script rather than skipping the row.
        g = [x for x in cv if x['duty'] == d and x['worst'] and x['hold']]
        if g:
            rr = [x['worst'] / x['hold'] for x in g]
            print(f'  duty {d:4d}  n={len(rr):3d}  ratio {min(rr):.3f}-{max(rr):.3f}'
                  f'  median {statistics.median(rr):.3f}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
