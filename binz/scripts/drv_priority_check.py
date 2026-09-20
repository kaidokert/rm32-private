"""Disabled NVIC preemption control and peer-priority test."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off
def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('finaloff boundary')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    rows=re.findall(r'^PRIORITY(?:CASE|CHECK) .*$',body,re.M)
    expected=['PRIORITYCASE comp=64 com=128 sequence=123 expected=123 disabled=1',
              'PRIORITYCASE comp=64 com=64 sequence=132 expected=132 disabled=1',
              'PRIORITYCHECK restored=1 gate_authority=0']*3
    if rows!=expected:raise ValueError('priority ordering/restoration failed: '+repr(rows))
    return dict(trials=3,unequal_preempts=True,peers_do_not_preempt=True,outputs_off=True)
def verify_dma(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('finaloff boundary')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    rows=re.findall(r'^DMAPRIORITY(?:CASE|CHECK) .*$',body,re.M)
    expected=['DMAPRIORITYCASE mode=1 comp=64 dma=0 guard=0 sequence=123 expected=123 disabled=1',
              'DMAPRIORITYCASE mode=1 comp=64 dma=64 guard=0 sequence=132 expected=132 disabled=1',
              'DMAPRIORITYCASE mode=2 comp=64 dma=64 guard=0 sequence=213 expected=213 disabled=1',
              'DMAPRIORITYCHECK restored=1 gate_authority=0']*3
    if rows!=expected:raise ValueError('DMA priority ordering/restoration failed: '+repr(rows))
    return dict(trials=3,dma_preempts_only_when_higher=True,pending_dma_before_comp=True,outputs_off=True)

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--dma',action='store_true');args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.4);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data)
            for _ in range(3):command('dmaprioritycheck' if args.dma else 'prioritycheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print((verify_dma if args.dma else verify)(data.decode()))
if __name__=='__main__':main()
