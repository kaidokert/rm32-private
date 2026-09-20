"""Disabled-only reset-policy measurement. Ten-us gate, not full ISR WCET."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line,read_available,verify_off

def verify(text,budget=False):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1: raise ValueError('missing finaloff')
    verify_off(text.split('FINALOFF\n')[1].encode())
    if budget:
        rows=re.findall(r'^IRQBUDGETCHECK ([^\r\n]+)',text,re.M)
        if rows!=['cases=5 failed=0 boundary_us=50 stopped_above=1 synthetic_elapsed=1 disabled=1 gate_authority=0']:
            raise ValueError('duration boundary/cleanup failed')
        return dict(cases=5,synthetic_elapsed=True,outputs_off=True)
    rows=re.findall(r'^SEEDCHECK trials=64 failed=0 reset_max_us=(\d+) disabled=1 full_isr=0 gate_authority=0$',text,re.M)
    if len(rows)!=1 or not 0<int(rows[0])<=10: raise ValueError('policy semantics/timing failed')
    return dict(trials=64,reset_max_us=int(rows[0]),full_isr=False,outputs_off=True)

if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--budget-check',action='store_true',help='test actual duration-stop boundary, not isolated reset cost')
    args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.5);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('irqbudgetcheck' if args.budget_check else 'seedcheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode(),args.budget_check))
