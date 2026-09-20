"""Join direct first-read buckets to accepted reference intervals, not rotor edges."""
import argparse
import hashlib
import json
from pathlib import Path
from drv_qualification_direct import decode_campaign
from drv_accepted_events import decode_windows
from drv_sustained_report import summarize
from drv_boundary_pairs import pairs


def join_rows(direct,windows):
    if direct['final_accepted']!=windows['total']:
        raise ValueError('direct/reference final identity mismatch')
    start=windows['total']-len(windows['tail'])
    result=[]
    for bucket in direct['rows']:
        index=bucket['accepted_before']-start
        if not 0<=index<len(windows['tail']):
            raise ValueError('direct row outside retained reference tail')
        event=windows['tail'][index]
        if bucket['step']!=event['step']:
            raise ValueError('direct/reference sector mismatch')
        first,last=bucket['first_open'],bucket['last_open']
        interval=event['reference_interval_ticks']
        result.append(dict(**bucket,reference_interval_ticks=interval,
                           reference_interval_us=interval/2,
                           first_to_last_open_us=(last-first)/2 if last>=first else None,
                           last_open_to_reference_us=(interval-last)/2 if interval>=last else None))
    return result


def same_sector_changes(joined):
    """Adjacent electrical visits only; no interpolation across missing rows."""
    by_id={row['accepted_before']:row for row in joined}
    result=[]
    for row in joined:
        previous=by_id.get(row['accepted_before']-6)
        if previous is None:continue
        if previous['step']!=row['step']:raise ValueError('same-sector six-event mismatch')
        result.append(dict(accepted_before=row['accepted_before'],previous_before=previous['accepted_before'],
                           step=row['step'],open_change=row['open']-previous['open'],
                           closed_change=row['closed']-previous['closed'],
                           first_open_change_us=(row['first_open']-previous['first_open'])/2,
                           last_open_change_us=(row['last_open']-previous['last_open'])/2,
                           reference_interval_change_us=(row['reference_interval_ticks']-previous['reference_interval_ticks'])/2))
    return result


def report(text):
    summary=summarize(text)  # Includes CRC, timeline and final-off checks.
    direct=decode_campaign(text,True)
    windows=decode_windows(text)
    joined=join_rows(direct,windows)
    by_id={r['accepted_before']:r for r in joined}
    ranked=pairs(windows['tail'],windows['total'])
    covered=[dict(pair=p,left=by_id[p['accepted_before']],right=by_id[p['accepted_before']+1])
             for p in ranked if p['accepted_before'] in by_id and p['accepted_before']+1 in by_id]
    return dict(outcome=summary['outcome'],epoch=direct['epoch'],joined=joined,
                same_sector_changes=same_sector_changes(joined),
                largest_covered_pairs=covered[:3],partial=direct['partial'],
                pair_candidates_outside_direct_tail=len(ranked)-len(covered),
                outputs_off_verified=summary['outputs_off_verified'],
                persistence_failures_counted=False,physical_edge_latency_measured=False,
                first_epoch_covered=False,causal_mechanism_proven=False)


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('captures',nargs='+',type=Path)
    for path in ap.parse_args().captures:
        raw=path.read_bytes()
        print(json.dumps(dict(capture=str(path),sha256=hashlib.sha256(raw).hexdigest(),
                              **report(raw.decode())),indent=2))
