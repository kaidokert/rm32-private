"""Disabled-only live comparator read-span check; not full ISR timing."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line,read_available,verify_off


def verify(text,call):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing finaloff')
    verify_off(text.split('FINALOFF\n')[1].encode())
    rows=re.findall(r'^READCADENCE call=([01]) trials=64 reads=12 min_us=(\d+) max_us=(\d+) disabled=1 actual_isr=0 gate_authority=0$',text,re.M)
    if len(rows)!=1:raise ValueError('missing read span')
    mode,minimum,maximum=map(int,rows[0])
    if mode!=int(call) or not 0<minimum<=maximum<=10:raise ValueError('read span mode/cost gate')
    return dict(call=call,min_us=minimum,max_us=maximum,actual_isr=False,outputs_off=True)


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True);ap.add_argument('--call',action='store_true')
    args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.5);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('readcadencecheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode(),args.call))


if __name__=='__main__':main()
