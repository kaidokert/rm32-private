"""Disabled-only timer/DMA aperture check; never starts or wakes the motor."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line,read_available,verify_off
from drv_driven_run import records

def verify(text):
    text=text.replace('\r','')
    blocks=re.findall(r'^PWMDMA reason=([^\n]+)\n(.*?)^PWMDMA END$',text,re.M|re.S)
    if len(blocks)!=2: raise ValueError('two complete DMA timing targets required')
    result=[]
    for (header,body),target in zip(blocks,[192,320]):
        s={k:int(v) for k,v in re.findall(r'(\w+)=(\d+)','reason='+header)}
        if not (s['reason']==0 and s['target']==target and s['n']==64
                and 6250<=s['elapsed_us']<=6700 and s['adc_scans']>=20
                and s['flags']==7 and s['stopped']==s['disabled']==s['source_tim1_cnt']==1
                and s['gate_authority']==0): raise ValueError('DMA timing/completion/safing contract failed')
        rows=records(body,'PM85',3)
        if len(rows)!=64 or [r[0] for r in rows]!=list(range(64)): raise ValueError('missing/unordered DMA data')
        values=[r[1]|r[2]<<16 for r in rows]
        if not all(target<=v<=target+32 for v in values): raise ValueError('DMA read outside declared 0.5us latency bound')
        result.append({'target':target,'min_cnt':min(values),'max_cnt':max(values),'adc_scans':s['adc_scans']})
    marker=text.rfind('FINALOFF\n')
    if marker<0: raise ValueError('missing finaloff')
    verify_off(text[marker:].encode())
    return result

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path);ap.add_argument('--infile',type=Path);args=ap.parse_args()
    if bool(args.out)==bool(args.infile): ap.error('select --out OR --infile')
    if args.infile: text=args.infile.read_text()
    else:
        with args.out.open('xb') as log:
            data=bytearray()
            try:
                with serial.Serial('COM41',115200,timeout=.05) as port:
                    try:
                        for cmd in ['off','p','i']:
                            send_line(port,cmd);data.extend(read_available(port,.2))
                        verify_off(data)
                        send_line(port,'pwmdmatiming');data.extend(read_available(port,1.0))
                    finally:
                        data.extend(b'FINALOFF\n')
                        for cmd in ['off','p','i','stack']:
                            send_line(port,cmd);data.extend(read_available(port,.2))
            finally: log.write(data)
        text=data.decode('ascii')
    print(verify(text))

if __name__=='__main__': main()
