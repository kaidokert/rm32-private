"""Separate accepted-event prefix/tail cycle statistics; never bridge the gap.

These overlapping same-sector measurements are NOT independent rotor periods
or an acceleration estimate through the unrecorded middle of a run.
"""
import argparse
import hashlib
import json
from pathlib import Path
import statistics

from drv_accepted_events import decode_windows
from drv_sustained_report import summarize


def window_cycles(rows):
    if any(b['us']<=a['us'] or b['step']!=a['step']%6+1
           for a,b in zip(rows,rows[1:])):
        raise ValueError('noncontiguous cycle window')
    if len(rows)<7:
        return None
    cycles=[b['us']-a['us'] for a,b in zip(rows,rows[6:])]
    median=statistics.median(cycles)
    return dict(first_us=rows[0]['us'],last_us=rows[-1]['us'],
                retained_events=len(rows),overlapping_cycles=len(cycles),
                min_us=min(cycles),median_us=median,max_us=max(cycles),
                reciprocal_median_ehz=1_000_000/median,
                independent_samples=False,physical_rotor_measurement=False)


def report(text):
    summary=summarize(text)  # CRC, timeline and outputs-off verification.
    windows=decode_windows(text)
    return dict(outcome=summary['outcome'],
                aggregate_ehz=summary['accepted_rate_ehz'],
                prefix=window_cycles(windows['prefix']),
                tail=window_cycles(windows['tail']),
                skipped_events=windows['skipped'],
                gap_interpolated=False,steady_state_proven=False,
                rejected_event_included=False,
                outputs_off_verified=summary['outputs_off_verified'])


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('captures',nargs='+',type=Path)
    args=ap.parse_args()
    for path in args.captures:
        raw=path.read_bytes()
        print(json.dumps(dict(capture=str(path),sha256=hashlib.sha256(raw).hexdigest(),
                              **report(raw.decode())),indent=2))
