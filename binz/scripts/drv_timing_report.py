"""Offline timing evidence for verified driven handoff/recovery captures.

IRQ wall maxima are partial instrumentation brackets, NOT CPU occupancy.
No UART access, firmware changes, or authority to widen running guards.
"""
import argparse
import hashlib
import json
import re
from pathlib import Path

from drv_driven_handoff import verify


def singleton(text, label, required):
    lines = re.findall(r'^' + re.escape(label) + r' ([^\r\n]+)', text, re.M)
    if len(lines) != 1:
        raise ValueError('missing/duplicate ' + label)
    pairs = re.findall(r'(\w+)=(\d+)', lines[0])
    fields = dict(pairs)
    if len(fields) != len(pairs) or not set(required) <= fields.keys():
        raise ValueError('missing/duplicate timing fields in ' + label)
    return {key: int(value) for key, value in fields.items()}


def seed_stages(text, arm_age_ticks):
    """Optional measured stage brackets, not exclusive work or removable delay."""
    rows=re.findall(r'^SEEDLAT(?: ([^\r\n]*))?$',text,re.M)
    if not rows:return None
    names=['entry','reset','feedback','guard','reference']
    pattern=' '.join(name+r'_ticks=(\d+)' for name in names)+r' half_us=1'
    match=re.fullmatch(pattern,rows[0]) if len(rows)==1 else None
    if not match:raise ValueError('invalid SEEDLAT provenance')
    ages=list(map(int,match.groups()))+[arm_age_ticks]
    if any(not 0<=x<65536 for x in ages) or ages!=sorted(ages):
        raise ValueError('invalid seed stage chronology')
    labels=['edge_to_entry','entry_to_reset','reset_to_feedback',
            'feedback_to_guard','guard_to_reference','reference_to_arm']
    return dict(brackets_us=dict(zip(labels,[(b-a)/2 for a,b in zip([0]+ages,ages)])),
                arm_age_us=arm_age_ticks/2,includes_instrumentation=True,
                exclusive_cost=False,removable_delay_proven=False)


def acquire_stages(text, seed_timing, *, reentry):
    rows=re.findall(r'^ACQUIRELAT(?: ([^\r\n]*))?$',text,re.M)
    if not rows:return None
    names=['edge','qualified','cleared','published','returned']
    pattern=' '.join(name+r'_ticks=(\d+)' for name in names)+r' half_us=1 recovery_only=1'
    match=re.fullmatch(pattern,rows[0]) if len(rows)==1 else None
    if not match or not reentry or seed_timing is None:
        raise ValueError('invalid ACQUIRELAT provenance')
    ticks=list(map(int,match.groups()))
    if ticks!=sorted(ticks) or any(not 0<=x<40000 for x in ticks):
        raise ValueError('invalid acquisition stage chronology')
    entry=seed_timing['brackets_us']['edge_to_entry']
    if ticks[1]-ticks[0]<40 or (ticks[-1]-ticks[0])/2>entry:
        raise ValueError('acquisition stages violate dwell/entry bounds')
    labels=['edge_to_qualified','qualified_to_clear','clear_to_publish','publish_to_return']
    brackets=dict(zip(labels,[(b-a)/2 for a,b in zip(ticks,ticks[1:])]))
    brackets['return_to_entry']=entry-(ticks[-1]-ticks[0])/2
    return dict(brackets_us=brackets,includes_instrumentation=True,
                exclusive_cost=False,removable_delay_proven=False)


def report(text, *, reentry=False):
    # Reuse complete chronology, CRC, electrical, deadline and final-off checks.
    result = verify(text, dropout=reentry, reentry=reentry)
    powered = result['powered']
    irq = singleton(text, 'COREEXTI', ['live', 'irq_calls', 'comp_max_us', 'com_max_us', 'synthetic'])
    guard = singleton(text, 'POWERPATH', ['isr_max_us', 'commit_max_us'])
    if irq['live'] != 1 or irq['synthetic'] != 0:
        raise ValueError('timing evidence must come from real live IRQs')
    elapsed = int(powered['observed_us'])
    if elapsed <= 0 or irq['irq_calls'] < int(powered['accepted_events']):
        raise ValueError('invalid observation duration/IRQ count')
    seed_rows = re.findall(
        r'^CORESEED assumed=0 source=measured_flying interval_ticks=(\d+) '
        r'edge_age_ticks=(\d+) remaining_arr=(\d+) arm_us=(\d+) ', text, re.M)
    if len(seed_rows) != 1:
        raise ValueError('missing/duplicate segment arm')
    _, arm_age, remaining, cost = map(int, seed_rows[0])
    transfer = result['transfer']
    stages=seed_stages(text,arm_age)
    return dict(
        segment='recovered' if reentry else 'initial',
        observed_us=elapsed,
        mean_ehz=float(powered['accepted_rate_ehz']),
        cycle_sigma_us=float(powered['cycle_sigma_us']),
        comp_calls=irq['irq_calls'],
        comp_calls_per_second=irq['irq_calls'] * 1_000_000 / elapsed,
        adc_scans_per_second=int(singleton(text, 'POWERFEEDBACK', ['scans'])['scans']) * 1_000_000 / elapsed,
        comp_bracket_max_us=irq['comp_max_us'],
        com_bracket_max_us=irq['com_max_us'],
        guard_bracket_max_us=guard['isr_max_us'],
        commit_bracket_max_us=guard['commit_max_us'],
        initial_arm_remaining_us=transfer['remaining_half_us'] / 2,
        initial_arm_spare_above_floor_us=transfer['remaining_half_us'] / 2 - 32,
        segment_arm_remaining_us=remaining / 2,
        segment_arm_spare_above_floor_us=remaining / 2 - 32,
        segment_arm_cost_us=cost,
        segment_arm_cost_spare_us=16 - cost,
        seed_stage_timing=stages,
        acquisition_stage_timing=acquire_stages(text,stages,reentry=reentry),
        original_deadline_spare_us=(int(powered['original_end_elapsed_us']) - int(powered['final_elapsed_us'])) if reentry else None,
        cpu_utilization_percent=None,
        exclusive_irq_time_measured=False,
        full_isr_entry_exit_measured=False,
        faster_operation_qualified=False,
        calibrated_current_measured=False,
    )


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('captures', type=Path, nargs='+')
    ap.add_argument('--reentry', action='store_true')
    args = ap.parse_args()
    rows = []
    for path in args.captures:
        raw = path.read_bytes()
        rows.append(dict(capture=str(path), sha256=hashlib.sha256(raw).hexdigest(),
                         **report(raw.decode(), reentry=args.reentry)))
    print(json.dumps(rows, indent=2))


if __name__ == '__main__':
    main()
