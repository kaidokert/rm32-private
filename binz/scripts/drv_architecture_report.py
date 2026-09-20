"""Validated workload accounting, not a throttle-to-CPU extrapolation."""
import argparse
import hashlib
import json
from pathlib import Path
from drv_timing_report import report as timing_report, singleton
from drv_cpu_meter import decode as cpu_decode, decode_roots
from drv_comp_paths import decode as paths_decode


def accounting(text):
    """Software-boundary exclusive partition, including observer perturbation."""
    cpu=cpu_decode(text)
    if not cpu['valid']:
        raise ValueError('architecture report requires valid measured IRQ accounting')
    exclusive=None
    if cpu['kind']=='per_vector':
        names=['foreground_unattributed','guard_tim6','comparator','commutation_tim16',
               'driven_sector_tim3','polling_tim7','adc_dma']
        exclusive=[dict(context=i,name=names[i],calls=cpu['calls'][i],
                        exclusive_us=cpu['partition_us'][i],
                        percent_elapsed=100*cpu['partition_us'][i]/cpu['elapsed_us'],
                        mean_exclusive_us_per_call=(cpu['partition_us'][i]/cpu['calls'][i]
                            if cpu['calls'][i] else None)) for i in range(1,7)]
    return dict(irq_union_percent=100*sum(cpu['partition_us'][1:])/cpu['elapsed_us'],
                accounting_kind=cpu['kind'],per_vector_exclusive_cost=exclusive,
                outermost_irq_attribution=decode_roots(text,cpu),
                probe_cost_measured=False,exception_overhead_separate=False)


def report(text, *, reentry=False):
    timing=timing_report(text,reentry=reentry)  # full powered/recovery proof first
    measured=accounting(text)
    duration=timing['observed_us']/1_000_000
    com=singleton(text,'POWERCOMMITS',['applied'])['applied']
    quality=singleton(text,'ACCEPTQUALITY',['events'])
    accepted=quality['events']
    calls=timing['comp_calls']
    paths=paths_decode(text)
    if paths is not None:
        if paths['dispatched']>calls or paths['accepted']!=accepted:
            raise ValueError('COMPPATH event/call accounting mismatch')
    if not com or not 0<accepted<=calls:
        raise ValueError('invalid controller event denominator')
    period_us=duration*1_000_000/com
    return dict(
        segment=timing['segment'],duration_s=duration,mean_ehz=timing['mean_ehz'],
        **measured,
        comp_calls_per_s=calls/duration,com_per_s=com/duration,
        accepted_per_s=accepted/duration,adc_scans_per_s=timing['adc_scans_per_second'],
        comp_calls_per_com=calls/com,nonaccepting_comp_calls=calls-accepted,
        nonaccepting_comp_fraction=(calls-accepted)/calls,
        mean_commutation_period_us=period_us,
        theoretical_64mhz_cycles_per_mean_commutation=period_us*64,
        com_wall_max_us=timing['com_bracket_max_us'],
        commit_wall_max_us=timing['commit_bracket_max_us'],
        comp_wall_max_us=timing['comp_bracket_max_us'],
        recovery_or_entry_arm_spare_us=timing['segment_arm_spare_above_floor_us'],
        comparator_paths=paths,
        blank_gate_calls=None if paths is None else paths['closed'],
        closed_gate_fraction_of_dispatched=(None if paths is None else
            paths['closed']/paths['dispatched']),
        open_no_accept_fraction_of_dispatched=(None if paths is None else
            paths['open_no_accept']/paths['dispatched']),
        decision_fractions_are_cpu_fractions=False,
        persistence_reject_calls=None,
        removable_irq_fraction=None,
        cpu_utilization_percent=None,foreground_is_idle=False,
        throttle_scaling_inferred=False,full_throttle_feasibility_proven=False,
        maxima_are_additive=False,
    )


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('captures',nargs='+',type=Path)
    ap.add_argument('--reentry',action='store_true')
    args=ap.parse_args()
    rows=[]
    for path in args.captures:
        raw=path.read_bytes()
        rows.append(dict(capture=str(path),sha256=hashlib.sha256(raw).hexdigest(),
                         **report(raw.decode().replace('\r',''),reentry=args.reentry)))
    print(json.dumps(rows,indent=2))


if __name__=='__main__':main()
