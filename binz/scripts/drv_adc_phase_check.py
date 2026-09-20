"""Idle DMA route check: no motor command; retain all raw attempts."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off

def verify(text,raw_capture=False):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing/duplicate finaloff')
    verify_off(text.split('FINALOFF\n')[1].encode())
    checks=re.findall(r'^PHASECHECK n=32 max_counter_us=([0-2]) passed=1 disabled=1 adc_load=0 gate_authority=0$',text,re.M)
    if len(checks)!=3 or len(re.findall(r'^PHASECHECK .*$',text,re.M))!=3:
        raise ValueError('phase route check failed/missing')
    raw=re.findall(r'^PHASERAW .*$',text,re.M)
    if (raw_capture or raw) and raw!=['PHASERAW captures=32 config_preserved=1 adc_load=0 authority=0']*3:
        raise ValueError('raw capture/DMA coexistence failed')
    return checks

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--out',type=Path,required=True);p.add_argument('--raw-capture',action='store_true');args=p.parse_args()
    with args.out.open('xb') as raw:
        data=bytearray()
        with serial.Serial('COM41',115200,timeout=.05) as port:
            def command(line):
                send_line(port,line);chunk=read_available(port,.4)
                data.extend(chunk);raw.write(chunk);raw.flush()
            try:
                for line in ['off','p','i']:command(line)
                verify_off(bytes(data))
                for _ in range(3):command('adcphasecheck')
            finally:
                data.extend(b'FINALOFF\n');raw.write(b'FINALOFF\n')
                for line in ['off','p','i']:command(line)
        checks=verify(data.decode(),args.raw_capture)
        print('3 checks passed; max counter microseconds:',checks)


if __name__=='__main__':main()
