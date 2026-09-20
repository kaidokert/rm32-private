"""Operator-authorized PSU-only AM32 response test; not lock qualification."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line,read_available


def decode_segments(data):
    heads=re.findall(rb'^AM32SEGS ([0-9a-f]{8}) ([0-9a-f]{8})\r?$',data,re.M)
    lines=[line.rstrip(b'\r') for line in data.splitlines() if line.startswith(b'AM32SEG ')]
    if len(heads)!=1:raise ValueError('missing/ambiguous segment header')
    count,overflow=(int(x,16) for x in heads[0])
    if overflow or count>8 or len(lines)!=count:raise ValueError('segment count/overflow')
    rows=[]
    for line in lines:
        m=re.fullmatch(rb'AM32SEG ((?:[0-9a-f]{8} ){6}[0-9a-f]{8})',line)
        if not m:raise ValueError('malformed segment')
        row=[int(x,16) for x in m[1].split()]
        inp,avg,zc,arr,ccr,running,latch=row
        if not 48<inp<=647 or arr>65535 or ccr>((arr+1)*3)//10 or running>1:
            raise ValueError('segment bounds')
        rows.append(row)
    return rows


def stop(port,raw,segments=None):
    send_line(port,'s')
    data=read_available(port,.7);raw.write(data);raw.flush()
    matches=re.findall(rb'AM32STOP ((?:[0-9a-f]{8} ){5}[0-9a-f]{8})\r?\n',data)
    if len(matches)!=1:raise ValueError('missing/ambiguous AM32 stop acknowledgement')
    row=[int(v,16) for v in matches[0].split()]
    if row[5]&2:raise ValueError('DRV ENABLE is not low')
    if segments is not None:segments.extend(decode_segments(data))
    return row


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--throttle',type=int,required=True,choices=range(1,31))
    ap.add_argument('--ramp-from',type=int,choices=range(1,31),help='ascending 5-point input increments, no stop between segments')
    ap.add_argument('--seconds',type=float,default=2.0)
    ap.add_argument('--segments',action='store_true',help='require command-boundary RAM snapshots')
    args=ap.parse_args()
    if not 0<args.seconds<=2.5:ap.error('single response window must precede 3s deadman')
    if args.ramp_from is not None and args.ramp_from>=args.throttle:ap.error('ramp must ascend')
    steps=list(range(args.ramp_from,args.throttle,5))+[args.throttle] if args.ramp_from is not None else [args.throttle]
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        raw.write(f'AM32RESPONSE throttle_input_percent={args.throttle} seconds={args.seconds} psu_only=1 pwm_cap_percent=30\n'.encode())
        stop(port,raw)
        try:
            during=b''
            for value in steps:
                raw.write(f'HOSTINPUT percent={value}\n'.encode());raw.flush()
                send_line(port,str(value))
                segment=read_available(port,args.seconds)
                during+=segment;raw.write(segment);raw.flush()
                if b'AM32STOP' in segment:break
        finally:
            segments=[]
            row=stop(port,raw,segments if args.segments else None)
        if args.segments:
            print(dict(command_boundary_snapshots=segments,atomic=False))
            if [r[0] for r in segments]!=[v*20+47 for v in steps[:-1]]:
                raise ValueError('segment input sequence mismatch')
        if b'AM32STOP' in during:raise ValueError('AM32 stopped before requested window ended')
        average,zc,arr,ccr,running,odr=row
        if ccr>((arr+1)*3)//10:raise ValueError('reported PWM compare exceeds 30%')
        print(dict(throttle_input_percent=args.throttle,average_interval_ticks=average,
                   estimated_ehz=2_000_000/(6*average) if average and running and zc>20 else None,
                   zero_crosses=zc,running=running,arr=arr,compare=ccr,
                   compare_percent=100*ccr/(arr+1),enable_off=not bool(odr&2),
                   independent_lock_proven=False))
        if not running or zc<=20:raise ValueError('no zero-cross progress beyond startup at stop')
        if args.segments and any(not r[5] or r[2]<=20 for r in segments):
            raise ValueError('one or more segment endpoints lacked running/progress')


if __name__=='__main__':main()
