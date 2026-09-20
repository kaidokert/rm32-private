"""Audit diagnostic detector sampling capacity, not BEMF quality or rotor lock.

The current detector needs four consecutive valid reads within a visit:
two opposite, then two expected. Fewer than four makes acceptance impossible
regardless of analog signal quality. Four is necessary, not sufficient.
"""
import argparse
import json
from pathlib import Path
from drv_observation import decode


def analyze(rows):
    visits = []
    for row in rows:
        if (not visits or row['step'] != visits[-1][-1]['step']
                or row['sector_us'] < visits[-1][-1]['sector_us']):
            visits.append([])
        visits[-1].append(row)
    result = []
    for index, visit in enumerate(visits):
        longest = streak = 0
        for row in visit:
            # Decision Invalid is the firmware's actual validity result. This
            # avoids reconstructing validity incorrectly across wire versions.
            valid = row['decision_reason'] != 0
            streak = streak + 1 if valid else 0
            longest = max(longest, streak)
        result.append(dict(step=visit[0]['step'], samples=len(visit),
                           max_valid_streak=longest,
                           capacity=longest >= 4,
                           complete=0 < index < len(visits)-1))
    complete = [v for v in result if v['complete']]
    return dict(complete_visits=len(complete),
                capacity_visits=sum(v['capacity'] for v in complete),
                max_valid_streak=max((v['max_valid_streak'] for v in complete), default=0),
                visits=result)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('captures', nargs='+', type=Path)
    parser.add_argument('--details', action='store_true')
    args = parser.parse_args()
    for path in args.captures:
        result = analyze(decode(path.read_text()))
        if not args.details:
            del result['visits']
        print(json.dumps(dict(capture=str(path), **result)))
