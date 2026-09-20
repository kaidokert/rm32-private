"""Descriptive two-cycle fault comparison, not a causal or rotor estimator."""
import argparse
import hashlib
import json
from pathlib import Path
from drv_cycle_fault import context
from drv_local_cycles import report as local_report


def report(text):
    fault=context(text)  # Validates CRC, tail continuity, fault and finaloff.
    local=local_report(text)
    reference=fault['reference_cycles']
    if reference is None or local['tail'] is None:
        raise ValueError('reference pair and contiguous local tail required')
    median=local['tail']['median_us']
    previous=reference['previous_cycle_ticks']/2
    current=reference['refused_cycle_ticks']/2
    return dict(step=fault['guard']['step'],decision_us=fault['guard']['decision_us'],
                guard_cycle_us=fault['guard']['delta_us'],
                reference_previous_us=previous,reference_current_us=current,
                local_recorder_median_us=median,
                previous_minus_local_us=previous-median,
                current_minus_local_us=current-median,
                pair_mean_minus_local_us=(previous+current)/2-median,
                guard_minus_reference_us=fault['clock_closure']['guard_minus_reference_us'],
                paired_sector_changes_us=[x/2 for x in reference['paired_interval_changes_ticks']],
                sector_order=reference['interval_steps'][6:],
                outputs_off_verified=fault['outputs_off_verified'],
                different_timestamp_brackets=True,selected_faults_only=True,
                independent_rotor=False,preemption_proven=False,
                pwm_quantization_proven=False)


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('captures',nargs='+',type=Path)
    args=ap.parse_args()
    for path in args.captures:
        raw=path.read_bytes()
        print(json.dumps(dict(capture=str(path),sha256=hashlib.sha256(raw).hexdigest(),
                              **report(raw.decode())),indent=2))
