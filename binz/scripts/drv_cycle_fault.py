"""Validate fault-only guard timestamps; recorder extrema are a different clock sample."""
import re


def decode(text):
    lines = re.findall(r'^CYCLEFAULT ([^\r\n]+)', text, re.M)
    if not lines:
        return None  # Legacy: the refused event was not captured.
    if len(lines) != 1:
        raise ValueError('duplicate CYCLEFAULT')
    fields = dict(re.findall(r'(\w+)=(\d+)', lines[0]))
    keys = {'step', 'previous_us', 'decision_us', 'delta_us', 'guard_timestamp'}
    if set(fields) != keys:
        raise ValueError('incomplete cycle fault snapshot')
    s = {k: int(v) for k, v in fields.items()}
    if s['guard_timestamp'] != 1 or any(v > 0xffffffff for v in s.values()):
        raise ValueError('invalid guard timestamp encoding')
    if s['step'] == 0 and all(s[k] == 0 for k in keys - {'guard_timestamp', 'step'}):
        return dict(available=False)  # Non-event CycleTiming refusal.
    if not 1 <= s['step'] <= 6 or (s['decision_us'] - s['previous_us']) % 2**32 != s['delta_us']:
        raise ValueError('cycle fault timestamp arithmetic')
    limits = re.findall(r'^RUNLIMIT cycle_min_us=(\d+) event_min_us=\d+ cycle_max_us=(\d+)', text, re.M)
    reasons = re.findall(r'^POWERPATH reason=(\d+)', text, re.M)
    if len(limits) != 1 or reasons != ['12']:
        raise ValueError('cycle fault lacks matching guard profile/reason')
    minimum, maximum = map(int, limits[0])
    if minimum <= s['delta_us'] <= maximum:
        raise ValueError('reported cycle does not violate guard')
    return dict(available=True, **s)


def core_snapshot(text):
    lines=re.findall(r'^CYCLECORE ([^\r\n]+)',text,re.M)
    if not lines:return None
    keys=('step','rising','average_ticks','previous_average_ticks','interval_ticks',
          'this_zc_ticks','last_zc_ticks','wait_ticks','filter','zero_crosses',
          'polling','running','after_safing','before_ev_acc_return')
    pattern=' '.join(k+r'=(\d+)' for k in keys)
    match=re.fullmatch(pattern,lines[0]) if len(lines)==1 else None
    if not match:raise ValueError('invalid cycle controller snapshot')
    s=dict(zip(keys,map(int,match.groups())))
    fault=decode(text)
    if (not fault or not fault['available'] or s['step']!=fault['step']
            or any(s[k]>0xffffffff for k in keys)
            or any(s[k]>1 for k in ('rising','polling','running'))
            or s['after_safing']!=1 or s['before_ev_acc_return']!=1):
        raise ValueError('cycle controller snapshot provenance')
    return s


def reference_cycles(tail, state):
    """Two adjacent cycles on the reference counter, including the refused edge.

    Counter resets/read latency are included in the firmware measurements;
    these sums are NOT independently measured rotor periods. Do not combine
    them with recorder/guard timestamps or bridge a missing window.
    """
    if state is None or len(tail)<11:
        return None
    rows=tail[-11:]
    if (state['polling'] or not state['running']
            or rows[-1]['step']%6+1!=state['step']
            or rows[-1]['reference_interval_ticks']!=state['last_zc_ticks']
            or any(b['step']!=a['step']%6+1 or b['us']<=a['us']
                   for a,b in zip(rows,rows[1:]))):
        raise ValueError('reference cycle tail/snapshot continuity')
    intervals=[r['reference_interval_ticks'] for r in rows]+[state['this_zc_ticks']]
    if any(not 0<x<=65535 for x in intervals):
        raise ValueError('reference cycle interval bounds')
    previous,current=sum(intervals[:6]),sum(intervals[6:])
    return dict(previous_cycle_ticks=previous,refused_cycle_ticks=current,
                two_cycle_mean_us=(previous+current)/4,
                paired_interval_changes_ticks=[b-a for a,b in zip(intervals[:6],intervals[6:])],
                interval_steps=[r['step'] for r in rows]+[state['step']],
                intervals_ticks=intervals,tick_us=0.5,
                reference_counter_only=True,independent_rotor_period=False)


def context(text):
    """Retained tail only. Never splice recorder timestamps into guard deltas."""
    from drv_accepted_events import decode_windows
    from drv_sustained_report import summarize
    summary = summarize(text)  # CRC, chronology and post-capture output-off checks.
    fault = decode(text)
    if not fault or not fault['available']:
        raise ValueError('exact cycle fault required')
    tail = decode_windows(text)['tail']
    cycles = {step: [] for step in range(1, 7)}
    for a, b in zip(tail, tail[6:]):
        if a['step'] != b['step']:
            raise ValueError('tail cycle sector mismatch')
        cycles[b['step']].append(b['us'] - a['us'])
    prior = [r for r in tail if r['step'] == fault['step']]
    state=core_snapshot(text)
    reference=reference_cycles(tail,state)
    closure=None
    if reference is not None:
        reference_us=reference['refused_cycle_ticks']/2
        closure=dict(guard_cycle_us=fault['delta_us'],reference_cycle_us=reference_us,
                     guard_minus_reference_us=fault['delta_us']-reference_us,
                     same_accepted_sector_span=True,independent_clocks=False,
                     is_isr_latency_measurement=False,
                     threshold_selected_failure=True,
                     pwm_quantization_proven=False)
    return dict(guard=fault, controller=state, reference_cycles=reference, clock_closure=closure, outcome=summary['outcome'],
                outputs_off_verified=summary['outputs_off_verified'],
                recorder_tail_cycles_us=cycles,
                last_same_sector_recorder_us=prior[-1]['us'] if prior else None,
                recorder_after_previous_guard_us=(prior[-1]['us']-fault['previous_us']) if prior else None,
                tail_only=True, rejected_event_recorded=False,
                physical_overspeed_proven=False, cause_identified=False)


if __name__ == '__main__':
    import argparse
    import json
    from pathlib import Path
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture',type=Path)
    args=parser.parse_args()
    print(json.dumps(context(args.capture.read_text()),indent=2))
