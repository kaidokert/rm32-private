"""Outputs-disabled TIM2 interrupt-source contract; never starts a motor."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line, read_available, verify_off

def verify(text,require_detail=False):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('finaloff boundary')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    rows=re.findall(r'^FILTERSOURCE .*$',body,re.M)
    expected='FILTERSOURCE bits=1023 visits=2 restored=1 disabled=1 gate_authority=0'
    if rows!=[expected]*3:raise ValueError('source contract, restoration or trial count: '+repr(rows))
    details=re.findall(r'^FILTERRESTORE .*$',body,re.M)
    if details or require_detail:
        if len(details)!=3:raise ValueError('restoration detail count')
        for row in details:
            m=re.fullmatch(r'FILTERRESTORE timer_diff=0 csr_saved=(\d+) csr_after=(\d+)',row)
            if not m:raise ValueError('timer restoration detail')
            before,after=map(int,m.groups())
            if max(before,after)>0xffffffff or (before^after)&~(1<<30):
                raise ValueError('comparator configuration changed')
    return dict(trials=3,checks_per_trial=10,irq_visits_per_trial=2,outputs_off=True)

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.4);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data)
            for _ in range(3):command('filtersourcecheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode(),require_detail=True))

if __name__=='__main__':main()
