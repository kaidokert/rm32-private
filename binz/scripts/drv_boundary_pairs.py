"""Rank retained adjacent long/short reference intervals, not physical delays.

Each candidate excludes its two rows from same-sector median baselines. Tail
only: no gap interpolation, no fault-event insertion, no frequency of faults.
"""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
from drv_accepted_events import decode_windows
from drv_sustained_report import summarize


def pairs(rows, total):
    if total < len(rows) or any(
        b['us'] <= a['us'] or b['step'] != a['step'] % 6 + 1
        for a, b in zip(rows, rows[1:])
    ):
        raise ValueError('noncontiguous tail or invalid total')
    if any(not 1 <= r['step'] <= 6 or
           not 0 < r['reference_interval_ticks'] <= 65535 for r in rows):
        raise ValueError('invalid sector/reference interval')
    result = []
    for i, (a, b) in enumerate(zip(rows, rows[1:])):
        baselines = []
        for row in (a, b):
            others = [r['reference_interval_ticks'] / 2
                      for j, r in enumerate(rows)
                      if j not in (i, i + 1) and r['step'] == row['step']]
            if len(others) < 2:
                break
            baselines.append(statistics.median(others))
        if len(baselines) != 2:
            continue
        d = a['reference_interval_ticks'] / 2 - baselines[0]
        e = b['reference_interval_ticks'] / 2 - baselines[1]
        result.append(dict(accepted_before=total-len(rows)+i, step=a['step'],
                           interval_delta_us=d, successor_delta_us=e,
                           pair_residual_us=d+e,
                           balanced_long_short_us=max(0, min(d, -e))))
    return sorted(result, key=lambda r: r['balanced_long_short_us'], reverse=True)


def report(text):
    summary = summarize(text)  # Validates full capture, CRC and outputs-off.
    windows = decode_windows(text)
    ranked = pairs(windows['tail'], windows['total'])
    return dict(outcome=summary['outcome'], retained_tail=len(windows['tail']),
                candidate_pairs=len(ranked), largest_pairs=ranked[:3],
                omitted_events=windows['skipped'],
                outputs_off_verified=summary['outputs_off_verified'],
                short_tail_baseline=True, independent_rotor=False,
                preemption_proven=False, population_rate_estimated=False)


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('captures', nargs='+', type=Path)
    for path in ap.parse_args().captures:
        raw = path.read_bytes()
        print(json.dumps(dict(capture=str(path), sha256=hashlib.sha256(raw).hexdigest(),
                              **report(raw.decode())), indent=2))
