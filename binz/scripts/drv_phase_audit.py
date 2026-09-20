"""Offline forward-sector edge audit. No live hardware access or control.

Expected raw polarity follows minz/src/comp2.rs::am32_change_comp_input:
minz_core::drive::edges_for(3, step-1), i.e. odd rm32 steps rise, even fall.
Do not transplant rm32_stm32's generic `rising` flag without its HAL polarity.
Support brackets are observation times, not exact analog zero-cross times.
"""
import argparse
import json
from pathlib import Path
from drv_crossings import sectors
from drv_observation import decode


def audit(records,signal='late_on'):
    visits=[r for r in records if r['signal']==signal]
    eligible=[r for r in visits if r['complete'] and r['edges']==1 and r['supported']==1]
    edges=[]
    for r in eligible:
        low,high,level=map(int,r['brackets'].split(':'))
        edges.append(dict(visit=r['visit'],step=r['step'],low_us=low,high_us=high,
                          absolute_low_us=r['start_us']+low,absolute_high_us=r['start_us']+high,
                          level=level,expected_level=r['step']&1,expected=level==(r['step']&1)))
    report=[]
    for step in range(1,7):
        chosen=[e for e in edges if e['step']==step]
        expected=[e for e in chosen if e['expected']]
        periods=[]
        for a,b in zip(expected,expected[1:]):
            # Only neighboring electrical cycles; do not bridge missing visits.
            if b['visit']-a['visit']!=6:continue
            minimum=b['absolute_low_us']-a['absolute_high_us']
            maximum=b['absolute_high_us']-a['absolute_low_us']
            if minimum>0:
                periods.append(dict(from_visit=a['visit'],to_visit=b['visit'],
                    period_us=[minimum,maximum],frequency_hz=[1e6/maximum,1e6/minimum]))
        report.append(dict(step=step,complete_visits=sum(r['complete'] and r['step']==step for r in visits),
            supported=len(chosen),expected_direction=len(expected),opposite_direction=len(chosen)-len(expected),
            crossing_brackets_us=[[e['low_us'],e['high_us']] for e in expected],same_step_periods=periods))
    return dict(signal=signal,steps=report,edges=edges,qualified=False,
        qualification_note='Direction agreement alone does not prove rotor tracking, analog validity, or complete crossing coverage.')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('captures',nargs='+',type=Path)
    args=parser.parse_args()
    for path in args.captures:
        report=audit(sectors(decode(path.read_text())))
        output=path.with_name(path.stem+'_phase_audit.json')
        output.write_text(json.dumps(report,indent=2)+'\n')
        print(path.name)
        for r in report['steps']:
            print('step',r['step'],'visits',r['complete_visits'],'expected/opposite',
                  r['expected_direction'],r['opposite_direction'],'brackets',r['crossing_brackets_us'],
                  'same_step_Hz',[[round(v,2) for v in p['frequency_hz']] for p in r['same_step_periods']])
        print('saved',output)


if __name__=='__main__':main()
