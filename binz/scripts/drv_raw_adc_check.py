"""Disabled raw-capture/ADC-DMA coexistence test; no motor commands."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off

def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('finaloff boundary')
    verify_off(text.split('FINALOFF\n')[1].encode())
    rows=re.findall(r'^RAWADC .*$',text,re.M)
    if len(rows)!=3:raise ValueError('raw ADC trial count')
    result=[]
    for row in rows:
        m=re.fullmatch(r'RAWADC scans=128 captures=128 fault=0 elapsed_us=(\d+) vref_min=(\d+) disabled=1 period_us=201 adc_channels=5 dma_polled=1 authority=0',row)
        if not m:raise ValueError('raw ADC coexistence failed')
        elapsed,vref=map(int,m.groups())
        if not 25528<=elapsed<30000 or not 0<vref<=4095:raise ValueError('raw ADC time/VREF invalid')
        result.append((elapsed,vref))
    return result

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,required=True);args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.4);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data)
            for _ in range(3):command('rawadccheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode()))
if __name__=='__main__':main()
