"""Disabled one-shot duty selector checks; no motor power requested."""
import argparse
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(line):
            send_line(port,line);chunk=read_available(port,.4)
            data.extend(chunk);raw.write(chunk);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data)
            command('dutycheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    text=data.decode().replace('\r','')
    verify_off(text.split('FINALOFF\n')[1].encode())
    expected='DUTYCHECK passed=6 expected=6 pending=0 disabled=1 gate_authority=0'
    if text.count(expected)!=1:raise ValueError('duty selector checks failed')
    print('6/6 duty selector checks passed; finaloff verified.')

if __name__=='__main__':main()
