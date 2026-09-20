"""Disabled timer arm cases; not a COM ISR WCET measurement."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line,read_available,verify_off


def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing finaloff')
    verify_off(text.split('FINALOFF\n')[1].encode())
    rows=re.findall(r'^COMARMCHECK trials=128 modes=4 arr_short=63 arr_long=399 failed=0 max_arm_us=(\d+) min_slack_us=(\d+) disabled=1 irq_masked=1 gate_authority=0$',text,re.M)
    if len(rows)!=1:raise ValueError('missing or failed timer cases')
    cost,slack=map(int,rows[0])
    if not 0<cost<=10 or not 20<=slack<=220:raise ValueError('timer arm cost/slack')
    return dict(trials=128,max_arm_us=cost,min_slack_us=slack,outputs_off=True)


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.5);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('comarmcheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode()))
