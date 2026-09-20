"""Join a final sparse call to a validated cycle refusal, without inferring latency."""
import argparse
import hashlib
import json
import statistics
from pathlib import Path
from drv_qualification_sparse import decode_campaign
from drv_cycle_fault import context
from drv_fault_pair import report as pair_report
from drv_accepted_events import decode_windows


def report(text):
    sparse=decode_campaign(text,required=True)
    fault=context(text)  # CRC, tail continuity, guard/controller and outputs-off.
    state=fault['controller']
    if not state or not sparse['rows']:
        raise ValueError('final sparse call and EV_ACC controller snapshot required')
    final=sparse['rows'][-1]
    if (not final['stopped'] or final['accepted']
            or final['step']!=state['step']
            or final['accepted_before']!=sparse['final_accepted']):
        raise ValueError('sparse final call does not match refused EV_ACC')
    windows=decode_windows(text)
    if windows['total']!=sparse['final_accepted']:
        raise ValueError('sparse binding disagrees with accepted log total')
    tail=windows['tail']
    if len(tail)<12 or tail[-6]['step']!=state['step']:
        raise ValueError('preceding same-sector boundary unavailable')
    # The guard compares the new (refused) boundary against this earlier one.
    # Reference intervals end at their labelled sector's accepted boundary.
    previous=tail[-6];successor=tail[-5]
    identity=windows['total']-6  # zero-based ACCEPTS before that earlier call
    baselines={step:statistics.median(r['reference_interval_ticks']/2
                                    for r in tail[:-6] if r['step']==step)
               for step in (previous['step'],successor['step'])}
    long_delta=previous['reference_interval_ticks']/2-baselines[previous['step']]
    short_delta=successor['reference_interval_ticks']/2-baselines[successor['step']]
    selected=[r for r in sparse['rows'] if r['accepted_before']==identity]
    # The ring preserves a suffix of selected calls in accepted-count order.
    # Strictly after its first identity is complete even if older rows were
    # overwritten. Equality is insufficient: same-count rejects may be lost.
    complete=(sparse['omitted_selected']==0 or
              identity>sparse['rows'][0]['accepted_before'])
    coverage=dict(accepted_before=identity,step=previous['step'],
                  interval_us=previous['reference_interval_ticks']/2,
                  successor_interval_us=successor['reference_interval_ticks']/2,
                  same_sector_baseline_us=baselines,
                  interval_minus_baseline_us=long_delta,
                  successor_minus_baseline_us=short_delta,
                  pair_residual_us=long_delta+short_delta,
                  selected_calls=selected,
                  selected_identity_coverage_complete=complete,
                  no_selected_call_proven=not selected and complete,
                  baseline_is_short_tail_estimate=True,
                  physical_edge_delay_measured=False)
    # CYCLECORE is emitted only from cycle_refusal_after_safing, reached by
    # powered_timer::accepted inside Obs::record(EV_ACC). Reference persistence
    # has succeeded; guard rejection prevents the later ACCEPTS increment.
    return dict(epoch=sparse['epoch'],final_call=final,preceding_boundary=coverage,
                reference_persistence_passed=True,guard_refused_event=True,
                accepted_flag_means='committed_ACCEPTS_increment_not_reference_persistence',
                dispatch_includes_safing=True,
                persistence_duration_us=None,guard_overlap_position=None,
                normal_call_overlap_rate=None,preemption_cause_proven=False,
                selected_successful_overlaps=sum(r['accepted'] and r['guard_overlap']
                                                for r in sparse['rows']),
                omitted_selected=sparse['omitted_selected'],pair=pair_report(text),
                outputs_off_verified=fault['outputs_off_verified'])


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('capture',type=Path)
    args=ap.parse_args()
    raw=args.capture.read_bytes()
    print(json.dumps(dict(capture=str(args.capture),sha256=hashlib.sha256(raw).hexdigest(),
                          **report(raw.decode())),indent=2))
