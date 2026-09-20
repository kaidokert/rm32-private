"""ENABLE-low comparator-to-TIM2 pulse filtering; no motor authority."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off

def verify(text,require_timing=False,require_t3=False,require_ack=False):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing finaloff boundary')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    if re.findall(r'^FILTERCHECK .*$',body,re.M)!=['FILTERCHECK restored=1 disabled=1 irq_enabled=0 gate_authority=0']:
        raise ValueError('filter restoration/authority check failed')
    rows=re.findall(r'^FILTERPART .*$',body,re.M)
    expected=[(0,2,1),(0,40,1),(15,2,0),(15,40,1)]
    if len(rows) in (6,8):expected.extend([(12,2,0),(12,40,1)])
    if len(rows)==8:expected.extend([(5,2,1),(5,40,1)])
    if len(rows)!=len(expected):raise ValueError('filter row count')
    result=[]
    for row,(code,width,captured) in zip(rows,expected):
        m=re.fullmatch(r'FILTERPART code=(\d+) requested_us=(\d+) measured_us=(\d+) levels=(\d+) captured=(\d+) overcapture=(\d+) ccr=(\d+) en_low=(\d+)',row)
        if not m:raise ValueError('filter malformed row')
        c,w,elapsed,levels,cap,over,ccr,en=map(int,m.groups())
        if (c,w,levels,cap,over,en)!=(code,width,2,captured,0,1):
            raise ValueError('filter capture/level/overcapture gate failed: '+row)
        if not width<=elapsed< (6 if code==12 and width==2 else 10 if width==2 else 50) or not 0<=ccr<=65535:
            raise ValueError('filter pulse width/counter range failed')
        result.append(dict(code=c,width_us=elapsed,captured=cap,ccr=ccr))
    timing=re.findall(r'^FILTERTIME .*$',body,re.M)
    if timing or require_timing:
        if len(timing)!=len(rows):raise ValueError('filter timing count')
        for index,(line,row) in enumerate(zip(timing,result)):
            m=re.fullmatch(r'FILTERTIME index=(\d+) before=(\d+) after=(\d+) elapsed_us=(\d+) post=(\d+)',line)
            if not m:raise ValueError('filter timing malformed')
            i,before,after,elapsed,post=map(int,m.groups())
            if i!=index or post!=0 or elapsed!=row['width_us'] or max(before,after)>65535:
                raise ValueError('filter timing state')
            delta=(after-before)&65535
            if abs(delta-2*elapsed)>4:raise ValueError('filter counter continuity')
            if row['captured']:
                latency=(row['ccr']-before)&65535
                low,high={0:(0,6),5:(1,6),12:(14,22),15:(28,38)}[row['code']]
                if not low<=latency<=high:raise ValueError('filter capture latency')
                row['latency_half_us']=latency
    pairs=re.findall(r'^FILTERPAIR .*$',body,re.M)
    if pairs:
        if len(pairs)!=len(rows):raise ValueError('filter pair count')
        for index,(line,row) in enumerate(zip(pairs,result)):
            m=re.fullmatch(r'FILTERPAIR index=(\d+) indirect_captured=(\d+) indirect_over=(\d+) indirect_ccr=(\d+) ic1f=0 cc1s=2',line)
            if not m:raise ValueError('filter pair format')
            i,cap,over,ccr=map(int,m.groups())
            if i!=index or cap not in (0,1) or over!=0 or ccr>65535:
                raise ValueError('filter pair state')
            if row['code']==0 and cap!=1:raise ValueError('filter pair unfiltered route failed')
            row.update(indirect_captured=cap,indirect_ccr=ccr)
    raw3=re.findall(r'^FILTERT3 .*$',body,re.M)
    if raw3 or require_t3:
        if len(raw3)!=len(rows):raise ValueError('filter TIM3 row count')
        for index,(line,row) in enumerate(zip(raw3,result)):
            m=re.fullmatch(r'FILTERT3 index=(\d+) captured=1 over=0 ccr=(\d+) before=(\d+) period=201 tick_us=1 filter=0',line)
            if not m:raise ValueError('filter TIM3 capture/format')
            i,ccr,before=map(int,m.groups())
            if i!=index or max(ccr,before)>200 or (ccr-before)%201>3:
                raise ValueError('filter TIM3 latency/range')
            row.update(raw3_delay_us=(ccr-before)%201)
    acks=re.findall(r'^FILTERACK .*$',body,re.M)
    if acks or require_ack:
        if len(acks)!=len(rows):raise ValueError('mirror acknowledgement count')
        for index,(line,row) in enumerate(zip(acks,result)):
            m=re.fullmatch(r'FILTERACK index=(\d+) before=(\d+) after_mirror=(\d+) after_source=(\d+) source_cc2=1 mirror_cc1=1',line)
            if not m:raise ValueError('mirror acknowledgement format')
            i,before,mirror,source=map(int,m.groups())
            expected=(6,4,0) if row['captured'] else (0,0,0)
            if i!=index or (before,mirror,source)!=expected:
                raise ValueError('mirror read consumed source event or own flag not cleared')
            row['mirror_ack_isolated']=True
    return result

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.4);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('filtercheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode(),require_timing=True,require_t3=True,require_ack=True))

if __name__=='__main__':main()
