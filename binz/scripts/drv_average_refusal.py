"""Opt-in startup with no current threshold must refuse before phase drive."""
import argparse
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off

def verify(text):
    expected='RUN refused: average current configuration missing; gates + en OFF'
    if text.count(expected)!=1 or 'RUN: align=' in text:
        raise ValueError('startup did not prove missing-threshold refusal')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing finaloff boundary')
    verify_off(text.split('FINALOFF\n')[1].encode())
    return {'missing_threshold_refused':True,'outputs_off':True}

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.5);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data)
            command('run200')
        finally:
            data.extend(b'FINALOFF\n');raw.write(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode().replace('\r','')))

if __name__=='__main__':main()
