"""Validate bounded-service evidence; wrapper test is not powered WCET."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off


def decode(text,required=False):
    rows=re.findall(r'^COMPCRITICAL(?: ([^\r\n]*))?$',text,re.M)
    if not rows and not required:return None
    m=re.fullmatch(r'calls=(\d+) max_us=(\d+) refused=(\d+) filter_required=12 limit_us=60 restores_between_calls=1 recorder_inside=1',rows[0]) if len(rows)==1 else None
    if not m:raise ValueError('critical service provenance missing/invalid')
    calls,maximum,refused=map(int,m.groups())
    if not 0<calls<=0xffffffff or not 0<maximum<=60 or refused:
        raise ValueError('critical service unused/refused/overrun')
    return dict(calls=calls,max_masked_body_us=maximum,limit_us=60,
                recorder_inside=True,prologue_epilogue_measured=False)


def verify_check(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing final off')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    rows=re.findall(r'^CRITICALCHECK (.*)$',body,re.M)
    if len(rows)!=4:raise ValueError('critical check row count/refusal')
    maxima=[]
    for mode,line in enumerate(rows):
        m=re.fullmatch(rf'mode={mode} passed=16 total=16 max_us=(\d+) invalid_filters_refused=1 restored=1 disabled=1 wrapper_only=1 gate_authority=0',line)
        if not m:raise ValueError('critical wrapper validation failed')
        maximum=int(m[1]);maxima.append(maximum)
        if not (61<=maximum<=100 if mode==3 else 0<=maximum<=60):
            raise ValueError('critical wrapper duration failed')
    return dict(maxima_us=maxima,wrapper_only=True,powered_wcet_proven=False)


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,required=True);args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.4);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('criticalcheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify_check(data.decode()))


if __name__=='__main__':main()
