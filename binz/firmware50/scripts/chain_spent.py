"""The measured per-crossing (wait, spent, left) distribution from the chain rings.

This is the measurement the campaign has been arguing around for four entries.
E213-E231 all subtracted `spent_max_us = 11` -- a saturated WHOLE-RUN maximum
whose own doc comment says it reads 11 in every capture "including every run
that ever latched a late arm", i.e. it is documented as unable to discriminate.
E217 accepted that it is "the wrong arbiter" and then E231 subtracted it again.

The chain rings carry the real thing, per crossing. `scripts/chain.py` cannot
read the archived captures -- it requires the 7-column post-E180 format with
TIM2 fine stamps and the archives have 6 columns -- which is plausibly why the
aggregate was argued from instead:

  pre-E180:  CHAIN 1 <crossing_us> <arm_us> <wait_us> <spent_us> <step> <flag>
  post-E180: CHAIN 1 <crossing_us> <crossing_fine> <arm_fine> <wait_us>
                     <spent_fine> <step> <flag>

`left = wait - spent` is the margin the arm is actually loaded with, and
`left == 0` is exactly what latches `Reason::LateArm`.
"""
import argparse
import collections
import glob
import os
import re


def parse(path):
    """(wait, spent, step) per accepted crossing, plus the run's own metadata."""
    text = open(path, encoding='utf-8', errors='replace').read()
    duty = re.search(r'target_duty_tenths=(\d+)', text)
    level = re.search(r'advance_level=(\d+)', text)
    late = re.search(r'late_arms=(\d+)', text)
    smax = re.search(r'spent_max_us=(\d+)', text)
    crc = re.search(r'elf_crc32\s+(\w+)', text)
    rows, fine = [], False
    for line in text.splitlines():
        f = line.split()
        if len(f) < 3 or f[0] != 'CHAIN' or f[1] != '1':
            continue
        c = f[2:]
        if len(c) == 6:          # pre-E180: crossing arm wait spent step flag
            wait, spent, step = int(c[2]), int(c[3]), int(c[4])
        elif len(c) == 7:        # post-E180, fine stamps in 15.6 ns ticks
            wait, spent, step = int(c[3]), int(c[4]), int(c[5])
            fine = True
        else:
            continue
        rows.append((wait, spent, step))
    return dict(duty=int(duty.group(1)) if duty else 0,
                level=int(level.group(1)) if level else 0,
                late=int(late.group(1)) if late else -1,
                spent_max=int(smax.group(1)) if smax else -1,
                crc=crc.group(1) if crc else '?',
                fine=fine, rows=rows)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--glob', default='captures/chain/*.txt')
    a = ap.parse_args()

    print(f'{"capture":26s} {"duty":>4} {"lvl":>3} {"n":>4}  '
          f'{"wait":>10} {"spent mode":>10} {"spent max":>9} '
          f'{"left mode":>9} {"left min":>8} {"left==0":>7} {"spent_max_us":>12}')
    tot = collections.Counter()
    arms = 0
    for f in sorted(glob.glob(a.glob)):
        d = parse(f)
        if not d['rows'] or d['fine']:
            continue          # fine-stamp images are a different unit; skip
        waits = collections.Counter(w for w, _, _ in d['rows'])
        spents = collections.Counter(s for _, s, _ in d['rows'])
        lefts = [max(w - s, 0) for w, s, _ in d['rows']]
        lc = collections.Counter(lefts)
        tot.update(spents)
        arms += len(d['rows'])
        print(f'{os.path.basename(f):26s} {d["duty"]:4d} {d["level"]:3d} '
              f'{len(d["rows"]):4d}  {waits.most_common(1)[0][0]:10d} '
              f'{spents.most_common(1)[0][0]:10d} {max(spents):9d} '
              f'{lc.most_common(1)[0][0]:9d} {min(lefts):8d} '
              f'{sum(1 for x in lefts if x == 0):7d} {d["spent_max"]:12d}')

    if not arms:
        raise SystemExit('no legacy-format CHAIN 1 rows found')
    print(f'\npooled over {arms} measured arms:')
    for s in sorted(tot):
        print(f'  spent = {s:2d} us : {tot[s]:5d}  ({100 * tot[s] / arms:5.1f}%)')
    mode = tot.most_common(1)[0][0]
    print(f'  modal spent {mode} us;  max observed {max(tot)} us')
    print('\nwait_time(ci, level) against these, at the projected 60% interval:')

    def adv(ci, l):
        l = min(l, 64)
        return (ci >> 6) * l + (((ci & 63) * l) >> 6)

    for ci in (64, 72, 77):
        row = f'  ci={ci:3d} us: '
        for l in (22, 20, 16):
            w = (ci >> 1) - adv(ci, l)
            row += (f'l={l}: wait {w:2d}, margin vs modal {w - mode:+3d} / '
                    f'vs worst {w - max(tot):+3d}   ')
        print(row)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
