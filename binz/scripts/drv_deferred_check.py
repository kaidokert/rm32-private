"""Disabled source/mask check; never commands motor drive."""
from pathlib import Path
import argparse
import serial
from drv_capture import send_line,read_available,verify_off

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    data=bytearray()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        def command(line):
            send_line(port,line);chunk=read_available(port,.5)
            data.extend(chunk);raw.write(chunk);raw.flush()
        try:
            for line in ['off','p','i']:command(line)
            verify_off(data)
            command('deferredcheck')
        finally:
            data.extend(b'FINALOFF\n');raw.write(b'FINALOFF\n')
            for line in ['off','p','i']:command(line)
    expected=b'DEFERREDCHECK cases=4 passed=1 no_owner=1 canceled=1 software_pending=1 cleared=1 disabled=1 gate_authority=0 active_owner_retry_tested=0'
    if data.count(expected)!=1:raise RuntimeError('deferred hardware source check failed')
    verify_off(data.split(b'FINALOFF\n')[1])
    print(dict(source_checks=4,outputs_off=True,active_owner_retry_tested=False))

if __name__=='__main__':main()
