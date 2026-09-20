"""Compare retained phase-current samples; never infer average PSU current.

Successful circular captures contain the END of a run, not its startup, so
they are not a matched-time control for failed startup captures.
"""
import argparse
import csv
import json
import re
import statistics
from pathlib import Path


def acquisition_context(path, count, last):
    """Recover the physical ring slot from the original CAP header, not tick.

    Current ADC order rotates with the ring slot, and resets on ring wrap.
    These are scan ordinals, NOT simultaneous samples or PWM timestamps.
    """
    transcript = path.with_suffix('.txt')
    if not transcript.exists():
        return None
    match = re.search(r'^CAP n=(\d+) capacity=(\d+) head=(\d+).*'
                      r'adc_order=rotating_ABC_BCA_CAB.*reason=(\d+)',
                      transcript.read_text(), re.MULTILINE)
    if not match:
        return None
    n, capacity, head, reason = map(int, match.groups())
    if not 0 < n <= capacity or not 0 <= head < capacity or n != count:
        raise ValueError('CAP header does not match retained CSV')
    slot = (head - 1) % capacity
    first = slot % 3
    phase = max(range(3), key=lambda p: abs(int(last[('ia', 'ib', 'ic')[p]]) - 2048))
    return dict(reason=reason, final_ring_slot=slot,
                final_adc_order=''.join('ABC'[(first + i) % 3] for i in range(3)),
                peak_phase_scan_ordinal=(phase - first) % 3 + 1,
                theta_u8=int(last['theta']),
                commanded_on=[int(last[k]) for k in ('on_a', 'on_b', 'on_c')])


def review(path):
    with path.open(newline='') as source:
        rows=list(csv.DictReader(source))
    if not rows:
        raise ValueError('empty capture')
    excursion=lambda r:max(abs(int(r[k])-2048) for k in ('ia','ib','ic'))
    before=rows[-51:-1]
    last=rows[-1]
    return dict(capture=path.name,n=len(rows),last_tick=int(last['tick']),
        stage=int(last['stage']),command_ehz=float(last['frequency_hz']),
        final_peak_phase=max(('ia','ib','ic'),key=lambda k:abs(int(last[k])-2048)),
        final_raw=[int(last[k]) for k in ('ia','ib','ic')],
        final_peak_counts=excursion(last),
        prior50_peak_counts=max(map(excursion,before),default=0),
        prior50_median_peak_counts=statistics.median(map(excursion,before)) if before else None,
        prior50_over1000=sum(excursion(r)>1000 for r in before),
        bus_min_mv=min(float(r['vbus_mv']) for r in rows),
        final_bus_mv=float(last['vbus_mv']),
        vref_range=[min(int(r['vref']) for r in rows),max(int(r['vref']) for r in rows)],
        flags_fault=sum(r['no_fault']=='False' for r in rows),
        acquisition=acquisition_context(path, len(rows), last))


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('captures',type=Path,nargs='+')
    args=p.parse_args()
    for path in args.captures:
        print(json.dumps(review(path)))
