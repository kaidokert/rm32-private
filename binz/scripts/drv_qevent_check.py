"""Disabled actual-Scope check with fixed 2/4 us cost gates, no motor."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line,read_available,verify_off


def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('final off framing')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    lines=re.findall(r'^QEVENTCHECK (.*)$',body,re.M)
    if len(lines)!=19 or lines[-1]!='END disabled=1':raise ValueError('event check refused/incomplete')
    maxima=[]
    for index,(preload,mode) in enumerate((p,m) for p in (0,16,32) for m in range(6)):
        m=re.fullmatch(rf'preload={preload} mode={mode} passed=16 total=16 max_us=(\d+) scope_only=1 gate_authority=0',lines[index])
        if not m:raise ValueError('event Scope lifecycle failure')
        cost=int(m[1]);limit=4 if mode in (2,3,4) else 2
        maxima.append(cost)
        if cost>limit:raise ValueError(f'event Scope cost failed: preload={preload} mode={mode} {cost}>{limit} us')
    return dict(maxima_us=maxima,lifecycle_passed=True,disabled_cost_gates_passed=True,
                reference_path_exercised=False,powered_wcet_proven=False)


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True);args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.8);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('qeventcheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode()))


if __name__=='__main__':main()
