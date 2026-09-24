"""Would the proposed SlowDroop (90/100, 4950-scan streak) have fired on the
only confirmed CC runs this bench has produced?

The adversarial review of E218 says no -- E201's runs sit at 929-951 permille,
ABOVE the 900 line -- which would make item 1 miss the exact condition it cites
as its motivation. Replayed here from the sag rings rather than taken on trust.

Also: find where E218's "1200/1215 = 0.987" came from, since the review says no
50% capture matches it.
"""
import glob
import os
import re

# The sag recorder's rows carry the guard's own inputs. Field names differ
# between the fast ring and the decimated one, so accept either.
ROW = re.compile(r'\b(?:bus_mean|bus)=(\d+)')


def ratios(path):
    t = open(path, encoding='utf-8', errors='replace').read()
    ref = re.findall(r'\bbus_ref=(\d+)', t)
    ref = int(ref[-1]) if ref else 0
    if not ref:
        return None
    means = []
    for line in t.splitlines():
        if line.strip().startswith(('SAGROW', 'SAGSLOW')):
            m = ROW.search(line)
            if m:
                means.append(int(m.group(1)))
    filt = re.findall(r'\bfilt_bus=(\d+)', t)
    return dict(ref=ref, n=len(means),
                worst=min(means) if means else None,
                filt=int(filt[-1]) if filt else None)


print('=== the confirmed-CC cohort (E201, 1.6 A, operator-confirmed clipping)')
for f in sorted(glob.glob('captures/**/e200-16a-s*.txt', recursive=True)):
    r = ratios(f)
    if not r:
        continue
    w = f'{1000 * r["worst"] // r["ref"]}' if r['worst'] else '--'
    fl = f'{1000 * r["filt"] // r["ref"]}' if r['filt'] else '--'
    print(f'  {os.path.basename(f):26s} ref={r["ref"]}  rows={r["n"]:5d}  '
          f'worst 8-scan={w} permille  filt={fl} permille  '
          f'{"WOULD TRIP" if r["worst"] and 1000 * r["worst"] // r["ref"] < 900 else "would NOT trip"}')

print('\n=== healthy 50% cohort, same statistic')
for f in sorted(glob.glob('captures/**/*500*.txt', recursive=True)):
    r = ratios(f)
    if not r or not r['worst']:
        continue
    print(f'  {os.path.basename(f):26s} ref={r["ref"]}  rows={r["n"]:5d}  '
          f'worst 8-scan={1000 * r["worst"] // r["ref"]} permille')

print('\n=== hunting E218\'s "1200/1215"')
hits = []
for f in sorted(glob.glob('captures/**/*.txt', recursive=True)):
    t = open(f, encoding='utf-8', errors='replace').read()
    if '1200' in t and re.search(r'\bbus_ref=1215\b', t):
        for line in t.splitlines():
            if '1200' in line:
                hits.append((os.path.basename(f), line.strip()[:110]))
for h in hits[:8]:
    print(f'  {h[0]:26s} {h[1]}')
if not hits:
    print('  no capture contains both 1200 and bus_ref=1215 -- the pair is not '
          'from a capture on this bench')
