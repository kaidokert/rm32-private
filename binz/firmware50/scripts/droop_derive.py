"""Derive a slow-droop line and its persistence from the existing corpus.

E218 chose a threshold from populations that did not exist in the captures (a
40% run read as 50%; a whole-run maximum read as a hold block) and both reviews
threw it out. E228 SS8 then showed the derivation needs no hardware at all: the
decision quantity is already recorded, on both populations, in the sag rings.
This reproduces that independently rather than taking it on trust.

What it reports:
  * the CV population -- healthy runs;
  * the CC population -- E201's operator-confirmed current-limited runs at the
    1.6 A clamp;
  * the worst CV row against the best CC row, so the margin on BOTH sides is
    explicit rather than inferred from an absence of trips (the E188 defect);
  * for each candidate line, the longest consecutive CV run below it, which is
    what a persistence constant has to clear.

The quantity is the one `FastBusSag` and the absolute floor judge: the
VREF-normalised bus against the PRE-RUN reference, as a cross-product. Not
`filt_bus`, which is the sharp guard's own 207 ms EWMA of its input and so
follows a slow droop instead of revealing it.

Two parsing scars, both mine, both of which produced a confident "no rows":
  * the ring rows are POSITIONAL, not `key=value` (layout at scripts/sag.py:10);
  * written through a shell heredoc, `\\b` in a non-raw string became an actual
    backspace character and every regex matched nothing. Edit the file, do not
    patch it through a heredoc.
"""
import argparse
import glob
import os
import re
from collections import defaultdict


def header(text):
    """Pre-run reference and image identity for one capture."""
    crc = re.search(r'elf_crc32\s+(\w+)', text)
    rb = re.findall(r'ref_bus=(\d+)', text) or re.findall(r'bus_ref=(\d+)', text)
    rv = re.findall(r'ref_vref=(\d+)', text)
    return (crc.group(1) if crc else '?',
            int(rb[-1]) if rb else 0,
            int(rv[-1]) if rv else 0)


def rows(path, include_slow=True):
    """(permille, duty_tenths) for every recorded guard judgement.

    SAGROW  <at> <bus_mean> <vref_mean> <filt_bus> <filt_vref> <streak> <step>
            <duty> <since_zc_us>
    SAGSLOW <bus_mean> <vref_mean> <filt_bus> <filt_vref>

    SAGSLOW carries no duty, so only SAGROW can be duty-conditioned.
    """
    text = open(path, encoding='utf-8', errors='replace').read()
    crc, ref_bus, ref_vref = header(text)
    if not ref_bus or not ref_vref:
        return crc, []
    out = []
    for line in text.splitlines():
        f = line.split()
        if not f:
            continue
        if f[0] == 'SAGROW' and len(f) >= 10:
            bus, vref, duty = int(f[2]), int(f[3]), int(f[8])
        elif include_slow and f[0] == 'SAGSLOW' and len(f) >= 5:
            bus, vref, duty = int(f[1]), int(f[2]), 0
        else:
            continue
        if vref == 0:
            continue
        out.append((bus * ref_vref * 1000 / (ref_bus * vref), duty))
    return crc, out


def longest_below(series, line):
    """Longest consecutive run strictly below `line`."""
    best = cur = 0
    for pm, _ in series:
        cur = cur + 1 if pm < line else 0
        best = max(best, cur)
    return best


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--root', default='captures')
    ap.add_argument('--cc-glob', default='*e200-16a-s[1-5].txt',
                    help="E201's operator-confirmed current-limited runs")
    ap.add_argument('--scan-hz', type=float, default=9901.0)
    a = ap.parse_args()

    files = sorted(glob.glob(os.path.join(a.root, '**', '*.txt'), recursive=True))
    cc_names = {os.path.basename(p) for p in
                glob.glob(os.path.join(a.root, '**', a.cc_glob), recursive=True)}

    pops = defaultdict(list)
    per_file = {}
    for f in files:
        base = os.path.basename(f)
        if 'warmup' in base:
            continue
        crc, series = rows(f)
        if not series:
            continue
        pop = 'CC' if base in cc_names else 'CV'
        per_file[base] = (pop, crc, series)
        for pm, duty in series:
            pops[pop].append((pm, duty, base))

    print(f'captures scanned: {len(files)};  with ring rows: {len(per_file)}')
    print('\n=== populations (recorded guard judgements, VREF-normalised per mille)')
    for pop in ('CV', 'CC'):
        v = sorted(x[0] for x in pops[pop])
        caps = sum(1 for p, _, _ in per_file.values() if p == pop)
        if not v:
            print(f'  {pop}: no rows found')
            continue
        n = len(v)
        worst = min(pops[pop], key=lambda x: x[0])
        print(f'  {pop}: n={n:6d} rows over {caps:3d} captures  '
              f'min={v[0]:.1f}  p1={v[n // 100]:.1f}  median={v[n // 2]:.1f}  '
              f'max={v[-1]:.1f}')
        print(f'        worst row {worst[0]:.1f} at duty {worst[1]} in {worst[2]}')

    if not pops['CV'] or not pops['CC']:
        raise SystemExit('both populations are needed to state a separation')

    cv_min = min(x[0] for x in pops['CV'])
    cc_max = max(x[0] for x in pops['CC'])
    print(f'\n=== separation: worst CV row {cv_min:.1f} vs best CC row {cc_max:.1f}')
    if cv_min > cc_max:
        print(f'  SEPARABLE on level alone: gap {cv_min - cc_max:.1f} per mille, '
              f'midpoint {(cv_min + cc_max) / 2:.1f}')
    else:
        print('  NOT SEPARABLE on level alone -- the populations overlap, so a '
              'level test cannot distinguish them and persistence must do it')

    print('\n=== candidate lines')
    print(f'  {"line":>5} {"CV rows<":>9} {"CV streak":>10} {"CV ms":>7}  '
          f'{"CC rows<":>9} {"CC caps":>9} {"CC streak":>10} {"CC ms":>7}')
    for line in (995, 990, 985, 980, 975, 970, 965, 960, 950, 900):
        cvb = sum(1 for pm, _, _ in pops['CV'] if pm < line)
        ccb = sum(1 for pm, _, _ in pops['CC'] if pm < line)
        cvs = max((longest_below(s, line)
                   for p, _, s in per_file.values() if p == 'CV'), default=0)
        ccs = max((longest_below(s, line)
                   for p, _, s in per_file.values() if p == 'CC'), default=0)
        hit = sum(1 for p, _, s in per_file.values()
                  if p == 'CC' and any(pm < line for pm, _ in s))
        tot = sum(1 for p, _, _ in per_file.values() if p == 'CC')
        print(f'  {line:5d} {cvb:9d} {cvs:10d} {1000 * cvs / a.scan_hz:7.1f}  '
              f'{ccb:9d} {f"{hit}/{tot}":>9} {ccs:10d} '
              f'{1000 * ccs / a.scan_hz:7.1f}')

    print('\n=== the duty->droop law, CV only, SAGROW rows (which carry duty)')
    by_duty = defaultdict(list)
    for pm, duty, _ in pops['CV']:
        if duty:
            by_duty[duty].append(pm)
    for duty in sorted(by_duty):
        v = sorted(by_duty[duty])
        print(f'  duty {duty:4d}  n={len(v):6d}  median={v[len(v) // 2]:7.1f}  '
              f'min={v[0]:7.1f}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
