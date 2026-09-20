"""Decode the CRC-framed spinning six-step microscope. No live hardware access."""
import base64
import csv
import pathlib
import re
import struct
import sys
import zlib

FIELDS='us_lo,us_hi,sector_us,step,float,cnt,flags,vc,neutral,ia,ib,ic,bus,vref,reject,cnt_end'.split(',')
PWM_SLOTS=[128,224,320,448,640,960,1600,2400,3200,4000,4800,5600]

def decode(text):
    match=re.search(r'OBS n=(\d+) wire=a85-v[12345678] fields=([^\r\n]+)\n(.*?)OBS END',text,re.S)
    if not match or match[2].split(',')!=FIELDS:
        raise ValueError('missing/invalid OBS frame')
    rows=[]
    for line in match[3].splitlines():
        if not line.startswith('B85 '):
            raise ValueError('unexpected observation record')
        raw=base64.a85decode(line[4:].encode())
        if len(raw)!=36 or zlib.crc32(raw[:32])!=struct.unpack('<I',raw[32:])[0]:
            raise ValueError('observation length/CRC')
        row=dict(zip(FIELDS,struct.unpack('<16H',raw[:32])))
        row['wire_version']=int(re.search(r'wire=a85-v(\d)',match[0])[1])
        row['us']=row['us_lo']|(row['us_hi']<<16)
        row['off']=row['flags']&1
        row['on']=(row['flags']>>1)&1
        row['late_on']=(row['flags']>>3)&1 if row['wire_version']>=4 else None
        row['analog_present']=not(row['wire_version']>=5 and row['flags']&16)
        row['c_minus_neutral']=row['vc']-row['neutral'] if row['analog_present'] else None
        if row['wire_version']>=6:
            if row['analog_present'] or row['vc']>4095 or row['neutral']>4095:
                raise ValueError('invalid PWM slot masks')
            for i,target in enumerate(PWM_SLOTS):
                row[f'pwm_{target}']=None if row['neutral']&(1<<i) else (row['vc']>>i)&1
            if row['neutral']&(1<<2):row['late_on']=None
            if row['neutral']&(1<<6):row['off']=None
        row['reference_late_on']=(None if row['late_on'] is None else
                                 row['late_on']^(row['wire_version']>=8))
        if row['wire_version']>=7:
            row['decision_expected']=(row['flags']>>5)&1
            row['decision_armed']=(row['flags']>>6)&1
            row['decision_event']=(row['flags']>>7)&1
            row['decision_latched']=(row['flags']>>8)&1
            row['decision_reason']=(row['reject']>>4)&7
            if row['decision_reason']>5:raise ValueError('invalid detector reason')
        if not 1<=row['step']<=6 or row['float']!=[2,0,1,2,0,1][row['step']-1]:
            raise ValueError('step/floating mismatch')
        if rows and row['us']<=rows[-1]['us']:
            raise ValueError('nonmonotonic timestamp')
        rows.append(row)
    if len(rows)!=int(match[1]):
        raise ValueError('observation count mismatch')
    return rows

def agreement(rows):
    # Only floating C has an analog cross-check. Exclude near-zero values;
    # 10 raw counts is an analysis deadband, not a calibrated uncertainty.
    selected=[r for r in rows if r['wire_version']>=4 and r['analog_present'] and r['float']==2
              and not r['reject']&11 and abs(r['c_minus_neutral'])>10]
    return dict(n=len(selected),early_match=sum(r['on']==int(r['c_minus_neutral']<0) for r in selected),
                late_match=sum(r['late_on']==int(r['c_minus_neutral']<0) for r in selected),
                early_late_changes=sum(r['on']!=r['late_on'] for r in selected))

def main():
    path=pathlib.Path(sys.argv[1])
    rows=decode(path.read_text())
    if not rows:
        raise ValueError('empty observation')
    output=path.with_name(path.stem+'_obs.csv')
    with output.open('w',newline='') as f:
        writer=csv.DictWriter(f,fieldnames=list(rows[0]));writer.writeheader();writer.writerows(rows)
    print('saved',output,'records',len(rows),'duration_us',rows[-1]['us'])
    print('reject_counts',{bit:sum(bool(r['reject']&bit) for r in rows) for bit in [1,2,4,8]})
    print('ADC_scan_end_counts',min(r['cnt_end'] for r in rows),max(r['cnt_end'] for r in rows))
    if rows[0]['wire_version']>=4: print('floating_C_sign_agreement',agreement(rows))
    if rows[0]['wire_version']>=6:
        print('PWM_slot_misses',{t:sum(r[f'pwm_{t}'] is None for r in rows) for t in PWM_SLOTS})
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    fig,axes=plt.subplots(6,1,figsize=(12,12),sharex=True)
    for step,ax in enumerate(axes,1):
        selected=[r for r in rows if r['step']==step and not(r['reject']&3)]
        ax.scatter([r['sector_us'] for r in selected],[r['on'] for r in selected],label='ON comparator',marker='x')
        if rows[0]['wire_version']>=4:
            late=[r for r in selected if r['late_on'] is not None]
            ax.scatter([r['sector_us'] for r in late],[r['late_on']-0.08 for r in late],label='late ON comparator',marker='+')
        off=[r for r in selected if r['off'] is not None]
        ax.scatter([r['sector_us'] for r in off],[r['off']+0.08 for r in off],label='OFF comparator',s=12)
        ax.set_ylabel(f'Step {step}');ax.set_ylim(-0.2,1.3);ax.grid()
    axes[0].legend();axes[-1].set_xlabel('Measured microseconds since commutation (all revolutions overlaid)')
    fig.tight_layout();fig.savefig(path.with_name(path.stem+'_obs.png'));plt.close(fig)
    for step in range(1,7):
        chosen=[r for r in rows if r['step']==step and not (r['reject']&(11 if r['wire_version']>=2 else 7))]
        analog=[r['c_minus_neutral'] for r in chosen if r['analog_present']]
        print('step',step,'valid',len(chosen),'on_states',sorted({r['on'] for r in chosen}),
              'late_states',sorted({r['late_on'] for r in chosen if r['late_on'] is not None}),
              'off_states',sorted({r['off'] for r in chosen if r['off'] is not None}),
              'C-N_range', (min(analog),max(analog)) if analog else None)

if __name__=='__main__':main()
