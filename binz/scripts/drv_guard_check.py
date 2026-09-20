"""Disabled timer faults and post-stop commit refusal; no motor command."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line, read_available, verify_off


def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing finaloff boundary')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    rows=re.findall(r'^POWERGUARD mode=(\d+) reason=(\d+) stop_us=(\d+) isr_max_us=(\d+) host_backstop=(\d+) disabled=(\d+) synthetic_feedback=1 gate_authority=0$',body,re.M)
    if len(rows)!=3:raise ValueError('missing guard cases')
    for expected,row in zip([4,8,1],rows):
        mode,reason,stop,cost,host,off=map(int,row)
        if mode!=[4,8,1].index(expected) or reason!=expected or host or off!=1 or not 0<stop<=1200 or cost>20:
            raise ValueError('guard timer fault mismatch')
    if re.findall(r'^POSTSTOP .*$',body,re.M)!=['POSTSTOP refused=6 expected=6 disabled=1']*3:
        raise ValueError('post-stop writes not refused')
    return dict(timer_faults=3,poststop_refusals=18,outputs_off=True)


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,required=True);args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.5);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('guardcheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode()))


if __name__=='__main__':main()
