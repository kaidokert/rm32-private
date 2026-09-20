"""Offline sector evidence, not a control algorithm or rotor-lock classifier.

Keep individual commutation visits separate. A supported transition needs two
adjacent valid samples on each side. Rejected samples break adjacency. Timing
is a bracket between observations, not interpolated zero-cross time. The first
and last visits are excluded because capture may truncate them.
"""
import argparse
import csv
from pathlib import Path
from drv_observation import decode


def sectors(rows):
    visits=[]
    for row in rows:
        if not visits or row['step']!=visits[-1][-1]['step'] or row['sector_us']<visits[-1][-1]['sector_us']:
            visits.append([])
        visits[-1].append(row)
    result=[]
    for visit,group in enumerate(visits):
        for signal in ['on','late_on','off']+[k for k in group[0] if k.startswith('pwm_')]:
            states=[]
            for row in group:
                mask=11 if row['wire_version']>=2 else 7
                # v6 has independent slot validity. Global bit2 brackets the
                # legacy late-ON read, not every OFF/other PWM slot.
                if signal.startswith('pwm_'):mask=1
                states.append(None if row['reject']&mask else row.get(signal))
            edges=[];supported=[]
            for i in range(1,len(states)):
                if states[i-1] is None or states[i] is None or states[i-1]==states[i]:
                    continue
                edge=(group[i-1]['sector_us'],group[i]['sector_us'],states[i])
                edges.append(edge)
                if i>=2 and i+1<len(states) and states[i-2]==states[i-1] and states[i+1]==states[i]:
                    supported.append(edge)
            result.append(dict(visit=visit,step=group[0]['step'],signal=signal,
                complete=0<visit<len(visits)-1,start_us=group[0]['us']-group[0]['sector_us'],
                samples=len(group),valid=sum(s is not None for s in states),
                states=''.join('-' if s is None else str(s) for s in states),
                edges=len(edges),supported=len(supported),
                brackets=';'.join(f'{a}:{b}:{v}' for a,b,v in supported)))
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('captures',nargs='+',type=Path)
    args=parser.parse_args()
    for path in args.captures:
        records=sectors(decode(path.read_text()))
        out=path.with_name(path.stem+'_crossings.csv')
        with out.open('w',newline='') as f:
            writer=csv.DictWriter(f,fieldnames=list(records[0]))
            writer.writeheader();writer.writerows(records)
        print(path.name)
        for signal in dict.fromkeys(r['signal'] for r in records):
            groups=[r for r in records if r['complete'] and r['signal']==signal and r['valid']]
            if not groups:continue
            print(signal,'complete_visits',len(groups),'no_transition',sum(r['edges']==0 for r in groups),
                  'multiple_transitions',sum(r['edges']>1 for r in groups),
                  'single_supported',sum(r['edges']==1 and r['supported']==1 for r in groups))
            print('single_supported_by_step', {s:sum(r['step']==s and r['edges']==1 and r['supported']==1 for r in groups) for s in range(1,7)})
        print('saved',out)


if __name__=='__main__':main()
