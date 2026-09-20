"""Idle SW/DMA/SW acquisition comparison, not current calibration."""
import argparse
import json
import re
from pathlib import Path
from drv_driven_run import records
from drv_capture import send_line,read_available,verify_off


def verify(text,settle_ms=None):
    text=text.replace('\r','')
    end=text.rfind('BASEMODE END')
    if end<0:raise ValueError('missing baseline comparison terminator')
    verify_off(text[end:].encode())
    settle=re.findall(r'^BASESETTLE ([^\n]+)',text,re.M)
    settled=None
    if settle:
        sm=re.fullmatch(r'target_us=(1000|20000) measured_us=(\d+) timer_sampled=1(?: fault_low_seen=([01]))?',settle[0]) if len(settle)==1 else None
        if not sm:raise ValueError('invalid settling metadata')
        target,measured=map(int,sm.groups()[:2])
        if not target<=measured<=target+200:raise ValueError('settling interval outside probe bounds')
        settled=dict(target_us=target,measured_us=measured,fault_low_seen=int(sm[3]) if sm[3] is not None else None)
    if settle_ms is not None and (not settled or settled['target_us']!=settle_ms*1000):
        raise ValueError('requested settling delay not applied')
    heads=re.findall(r'^BASEMODE ([^\n]+)',text,re.M)
    heads=[h for h in heads if h!='END']
    pattern=(r'complete=1 fault=0 same_wake=1 gates_off=1 n_target=128 '
             r'period_us=(101|201) first_us=(1|101) stationary_verified=0 offsets_applied=0 dma_polled=1')
    match=re.fullmatch(pattern,heads[0]) if len(heads)==1 else None
    if not match:raise ValueError('incomplete/invalid baseline comparison')
    period,first=map(int,match.groups())
    parts=re.findall(r'^BASEMODEPART mode=(\d+) elapsed_us=(\d+)$',text,re.M)
    if [int(p[0]) for p in parts]!=[0,1,2] or any(not 0<int(p[1])<50000 for p in parts):
        raise ValueError('baseline comparison timing/parts')
    if int(parts[1][1])<first+127*period:raise ValueError('DMA scan cadence too short')
    rows=records(text,'BM85',11)
    if len(rows)!=15:raise ValueError('baseline comparison row count')
    means=[[],[],[]]
    for ix,r in enumerate(rows):
        mode,ch,n,lo,hi,s0,s1,s2,s3,minimum,maximum=r
        total=lo+(hi<<16);squares=s0+(s1<<16)+(s2<<32)+(s3<<48)
        if (mode!=ix//5 or ch!=[4,1,0,6,13][ix%5] or n!=128
                or not 0<=minimum<=maximum<=4095 or not n*minimum<=total<=n*maximum
                or total*total>n*squares or not n*minimum**2<=squares<=n*maximum**2):
            raise ValueError('baseline comparison channel/count/moments')
        means[mode].append(total/n)
    return dict(means_counts=means,elapsed_us=[int(p[1]) for p in parts],settling=settled,
                dma_minus_sw_midpoint_counts=[means[1][i]-(means[0][i]+means[2][i])/2 for i in range(5)],
                sw_after_minus_before_counts=[means[2][i]-means[0][i] for i in range(5)],
                calibrated_current=False,drift_correction_proven=False,amps=None,outputs_off_verified=True)


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    group=ap.add_mutually_exclusive_group(required=True)
    group.add_argument('--out',type=Path);group.add_argument('--infile',type=Path)
    ap.add_argument('--settle-ms',type=int,choices=[1,20],default=1)
    args=ap.parse_args()
    if args.infile:text=args.infile.read_text()
    else:
        import serial
        with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
            data=bytearray()
            def command(cmd,delay=.3):
                send_line(port,cmd);chunk=read_available(port,delay);data.extend(chunk);raw.write(chunk);raw.flush()
            try:
                for cmd in ['off','p','i']:command(cmd)
                verify_off(bytes(data))
                command('basemode20' if args.settle_ms==20 else 'basemode',1.0)
            finally:
                raw.write(b'\nFINALOFF\n');data.extend(b'\nFINALOFF\n')
                for cmd in ['off','p','i']:command(cmd)
            text=data.decode(errors='replace')
    print(json.dumps(verify(text,args.settle_ms if args.out else None),indent=2))


if __name__=='__main__':main()
