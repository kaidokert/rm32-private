"""Disabled Scope checks; fixed instrumentation gates, no motor command."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off

LIMITS=(4,2,4,4,4)  # mode1 is the frequent nonaccepting path

def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing finaloff boundary')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    rows=re.findall(r'^QUALCHECK mode=(\d+) passed=(\d+) total=16 max_us=(\d+) scope_only=1 gate_authority=0$',body,re.M)
    if len(rows)!=5 or re.findall(r'^QUALCHECK END .*$',body,re.M)!=['QUALCHECK END disabled=1']:
        raise ValueError('missing qualification checks')
    maxima=[]
    for expected,row in enumerate(rows):
        mode,passed,cost=map(int,row)
        if mode!=expected or passed!=16:raise ValueError('qualification semantics failed')
        maxima.append(cost)
    if any(cost>limit for cost,limit in zip(maxima,LIMITS)):
        raise ValueError(f'qualification overhead refused: maxima={maxima}, limits={LIMITS}')
    return dict(max_us=maxima,limits_us=LIMITS,outputs_off=True,scope_only=True)

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True);args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.5);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('qualcheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode()))

if __name__=='__main__':main()
