"""Disabled actual decision Scope check. Timing is reported, not qualified."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line,read_available,verify_off


def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('final off framing')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    lines=re.findall(r'^DECISIONCHECK (.*)$',body,re.M)
    if len(lines)!=6 or lines[-1]!='END disabled=1':raise ValueError('decision check refused/incomplete')
    maxima=[]
    for mode,line in enumerate(lines[:-1]):
        m=re.fullmatch(rf'mode={mode} passed=16 total=16 max_us=(\d+) scope_only=1 gate_authority=0',line)
        if not m or int(m[1])>65535:raise ValueError('decision lifecycle failure')
        maxima.append(int(m[1]))
    return dict(maxima_us=maxima,lifecycle_passed=True,timing_qualified=False,
                reference_path_exercised=False)


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True);args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.4);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('decisioncheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode()))


if __name__=='__main__':main()
