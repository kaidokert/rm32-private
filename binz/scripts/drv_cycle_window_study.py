"""Offline speed-window study. Never changes or reclassifies firmware guards."""
import argparse
import hashlib
import json
from pathlib import Path
from drv_cycle_fault import context
from drv_sustained_report import fields


def first_refusal(events, floor_us=2778, cycles=1):
    """Synthetic monotonic-us events. Not firmware replay or a rotor model.

    Keep sector order,238..1000us spacing and6000us single slow-cycle checks.
    During two-cycle warmup retain the single-cycle floor. Missing events
    still require an external poll watchdog; a finite list cannot prove one.
    """
    if cycles not in (1,2) or floor_us<=0:raise ValueError('invalid study policy')
    for i,(at,step) in enumerate(events):
        if step not in range(1,7):return dict(index=i,reason='order')
        if i:
            prior,sector=events[i-1]
            if step!=sector%6+1:return dict(index=i,reason='order')
            gap=at-prior
            if gap>1000:return dict(index=i,reason='stale')
            if gap<238:return dict(index=i,reason='fast_event')
        if i>=6:
            one=at-events[i-6][0]
            if one>6000:return dict(index=i,reason='slow_cycle')
            n=2 if cycles==2 and i>=12 else 1
            span=at-events[i-6*n][0]
            if span<n*floor_us:return dict(index=i,reason='cycle_floor')
    return None


def fault_assessment(text):
    c=context(text)  # Original failure, CRC/chronology/finaloff validated.
    r=c['reference_cycles']
    if r is None:raise ValueError('missing refused reference interval history')
    floor=int(fields(text,'RUNLIMIT')['cycle_min_us'])
    one=r['refused_cycle_ticks']/2
    mean=r['two_cycle_mean_us']
    return dict(original_outcome=c['outcome'],cycle_floor_us=floor,
                reference_one_cycle_us=one,reference_two_cycle_mean_us=mean,
                single_below_floor=one<floor,two_cycle_below_floor=mean<floor,
                outputs_off_verified=c['outputs_off_verified'],
                local_counterfactual_only=True,whole_run_replayed=False,
                guard_clock_equivalence_proven=False,physical_lock_proven=False,
                safe_to_deploy=False)


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('captures',type=Path,nargs='+');args=ap.parse_args()
    for p in args.captures:
        raw=p.read_bytes()
        print(json.dumps(dict(file=str(p),sha256=hashlib.sha256(raw).hexdigest(),
                             **fault_assessment(raw.decode())),indent=2))
