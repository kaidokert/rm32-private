"""Early coast full-period bounds; conditional on timing uncertainty and no missed edges.

No polynomial extrapolation, controller-fed edge selection, or qZC certificate.
Only first32 rows have per-comparator completion offsets in current captures.
"""
import argparse
import json
from pathlib import Path
import re
from drv_capture import parse_dump
from drv_sustained_report import summarize


def periods(samples):
    """Samples are (read_lower_us, read_upper_us, level), in acquisition order."""
    edges=[]
    for previous,current in zip(samples,samples[1:]):
        if current[0]<=previous[1]: raise ValueError('overlapping/unordered read bounds')
        if current[2]!=previous[2]: edges.append((previous[0],current[1],current[2]))
    result=[]
    for first,last in zip(edges,edges[2:]):
        if first[2]!=last[2]: raise ValueError('nonalternating edges')
        lower=last[0]-first[1];upper=last[1]-first[0]
        if lower<=0: raise ValueError('unresolved full period')
        result.append(dict(start_lower_us=first[0],end_upper_us=last[1],
                           period_lower_us=lower,period_upper_us=upper,
                           frequency_lower_hz=1e6/upper,frequency_upper_hz=1e6/lower))
    return result


def period_spans(samples):
    """Longest same-polarity spans: cycle-count / elapsed bounds, not a fit.

    Allows varying speed. Still conditional on valid transitions/no missed
    edges; never infer an edge using the powered controller's expected rate.
    """
    edges=[]
    for previous,current in zip(samples,samples[1:]):
        if current[0]<=previous[1]:raise ValueError('overlapping/unordered read bounds')
        if current[2]!=previous[2]:edges.append((previous[0],current[1],current[2]))
    result=[]
    for first_index in (0,1):
        cycles=(len(edges)-1-first_index)//2
        if cycles<1:continue
        first=edges[first_index];last=edges[first_index+2*cycles]
        lower=last[0]-first[1];upper=last[1]-first[0]
        if lower<=0 or first[2]!=last[2]:raise ValueError('unresolved same-polarity span')
        result.append(dict(cycles=cycles,start_lower_us=first[0],end_upper_us=last[1],
                           elapsed_lower_us=lower,elapsed_upper_us=upper,
                           average_frequency_lower_hz=cycles*1e6/upper,
                           average_frequency_upper_hz=cycles*1e6/lower))
    return result


def analyze(text,uncertainty_us):
    if not 0<=uncertainty_us<=250: raise ValueError('uncertainty outside0..250us')
    run=summarize(text) # CRC, final off and powered evidence first
    if run['outcome']!='powered_window_complete': raise ValueError('no completed powered window')
    result=coast_bounds(text,uncertainty_us)
    controller=float(run['accepted_rate_ehz'])
    return dict(controller_rate_ehz=controller,read_uncertainty_us=uncertainty_us,
                phases=result,all_early_period_lower_bounds_above_half_controller=
                all(p['frequency_lower_hz']>controller/2 for r in result for p in r['periods']),
                lock_proven=False,qzc_measured=False,
                caveat='Conditional on supplied timestamp uncertainty, valid comparator transitions and no missed edges; coast speed is not shutdown speed or whole-run rotor tracking.')

def coast_bounds(text,uncertainty_us):
    """Measured coast only; caller separately verifies its actual drive mode."""
    if not 0<=uncertainty_us<=250: raise ValueError('uncertainty outside0..250us')
    _,_,rows,_,_,_=parse_dump(text)
    offsets=re.findall(r'^COASTCOMP row=(\d+) a_us=(\d+) b_us=(\d+) c_us=(\d+) from_scan_start=1$',text,re.M)
    if len(offsets)!=32 or [int(r[0]) for r in offsets]!=list(range(32)):
        raise ValueError('missing/duplicate early comparator timing')
    result=[]
    for phase in range(3):
        samples=[]
        for row,offset in zip(rows,offsets):
            if 'elapsed_us' not in row: raise ValueError('measured coast timestamps required')
            a,b,c=map(int,offset[1:])
            if not 0<a<b<c<1000: raise ValueError('invalid comparator offsets')
            # Read happened after prior comparator completion (or scan entry
            # for A), but before its own completion timestamp. Additional
            # uncertainty explicitly covers caller-to-scan skew/quantization.
            lo=row['elapsed_us']+[0,a,b][phase]-uncertainty_us
            hi=row['elapsed_us']+[a,b,c][phase]+uncertainty_us
            samples.append((lo,hi,bool(row['flags']&(2<<phase))))
        p=periods(samples)
        if len(p)<2: raise ValueError('insufficient early full periods')
        result.append(dict(phase='ABC'[phase],periods=p,spans=period_spans(samples)))
    return result


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('capture',type=Path)
    ap.add_argument('--uncertainty-us',required=True,type=int)
    args=ap.parse_args()
    print(json.dumps(analyze(args.capture.read_text(),args.uncertainty_us),indent=2))
