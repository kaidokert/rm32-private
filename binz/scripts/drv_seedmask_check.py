"""Disabled masked-seed setup check; not powered bootstrap or latency proof."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off

MARKER='SEEDMASK setup_masked=1 software_latch=1 bootstrap_enable_unchanged=1 age_floor_unchanged=1'

def verify_mode(text):
    if re.findall(r'^SEEDMASK .*$',text.replace('\r',''),re.M)!=[MARKER]:
        raise ValueError('masked seed setup provenance missing/invalid')

def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing/duplicate finaloff')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    expected='SEEDMASKCHECK passed=6 total=6 pending_request_injected=1 enable_primitive_only=1 disabled=1 gate_authority=0'
    if re.findall(r'^SEEDMASKCHECK.*$',body,re.M)!=[expected]*3:
        raise ValueError('masked seed register check failed')
    return dict(trials=3,sectors_per_trial=6,outputs_off=True,bootstrap_isr_tested=False)

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True);args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(line):
            send_line(port,line);chunk=read_available(port,.4)
            data.extend(chunk);raw.write(chunk);raw.flush()
        try:
            for line in ['off','p','i']:command(line)
            verify_off(data)
            for _ in range(3):command('seedmaskcheck')
        finally:
            data.extend(b'FINALOFF\n');raw.write(b'FINALOFF\n')
            for line in ['off','p','i']:command(line)
    print(verify(data.decode()))

if __name__=='__main__':main()
