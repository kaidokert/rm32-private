"""Recompute the worst 10.1 ms current block at the 50% rung, and the proxy's
zero drift, because E218 quoted 2116 mA and the evidence review says 2365."""
import glob
import os
import re

best = []
for f in sorted(glob.glob('captures/**/*.txt', recursive=True)):
    t = open(f, encoding='utf-8', errors='replace').read()
    crc = re.search(r'#\s*elf_crc32\s+(\w+)', t)
    for m in re.finditer(r'BEMFCURRENT ([^\n]*)', t):
        d = dict(re.findall(r'(\w+)=(-?\d+)', m.group(1)))
        if 'worst_ma' in d and d.get('duty_tenths') == '500':
            seg = t[max(0, m.start() - 4000):m.start() + 200]
            rs = re.search(r'reason=(\d+)', seg)
            best.append((int(d['worst_ma']), int(d.get('hold_ma', 0)),
                         int(d.get('zero_drift_ma', 0)),
                         crc.group(1) if crc else '?',
                         rs.group(1) if rs else '?',
                         os.path.basename(f)))
best.sort(reverse=True)
print(f'{"worst_ma":>8} {"hold_ma":>7} {"zdrift":>7}  {"image":9} rsn  capture')
for b in best[:10]:
    print(f'{b[0]:8d} {b[1]:7d} {b[2]:7d}  {b[3]:9s} {b[4]:3s}  {b[5]}')
w = [b[0] for b in best if b[0] < 100000]
z = [abs(b[2]) for b in best]
print(f'\n50% rung, n={len(best)} ({len(best) - len(w)} implausible discarded)')
print(f'  worst_ma {min(w)}-{max(w)};  |zero_drift_ma| up to {max(z)}')
print(f'  a 2600 mA ceiling is +{100 * 2600 / max(w) - 100:.1f}% over the worst '
      f'observed block ({max(w)} mA)')
print(f'  headroom {2600 - max(w)} mA vs worst drift {max(z)} mA -> '
      f'{"INSIDE the proxy error" if 2600 - max(w) < max(z) else "outside"}')
