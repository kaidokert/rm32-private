"""Finite stopped-buffer scan probe; no powered ADC ownership or mean-current claim."""
import argparse
import base64
from pathlib import Path
import re
import struct
import zlib
from drv_capture import send_line,read_available,verify_off


def decode(text):
    heads=re.findall(r'^ADCSCAN result=(\d+) scans=32 words=160 elapsed_us=(\d+) remaining=(\d+) stopped=(\d+) order=0,1,4,6,13 trigger_us=101 gate_authority=0$',text,re.M)
    if len(heads)!=1: raise ValueError('missing/duplicate scan header')
    reason,elapsed,remaining,stopped=map(int,heads[0])
    if reason or remaining or stopped!=1 or not 3000<=elapsed<=4000:
        raise ValueError('incomplete or live DMA scan')
    rows=[]
    for payload in re.findall(r'^AS85 (.*)$',text,re.M):
        raw=base64.a85decode(payload.strip())
        if len(raw)!=16 or zlib.crc32(raw[:12])!=int.from_bytes(raw[12:],'little'):
            raise ValueError('scan CRC/length')
        index,c,b,a,bus,vref=struct.unpack('<6H',raw[:12])
        if index!=len(rows) or max(a,b,c,bus,vref)>4095 or not 0<vref<4095:
            raise ValueError('scan order/raw range')
        rows.append(dict(a=a,b=b,c=c,bus=bus,vref=vref))
    if len(rows)!=32: raise ValueError('scan count')
    verify_off(text[text.rfind('\nAS85 '):].encode())
    return dict(elapsed_us=elapsed,scans=rows,logical_order='A=ADC4 B=ADC1 C=ADC0',
                measurement_accuracy_qualified=False)


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path);ap.add_argument('--input',type=Path)
    args=ap.parse_args()
    if args.input: result=decode(args.input.read_text())
    else:
        if args.out is None or args.out.exists(): ap.error('new --out path required')
        import serial
        raw=b''
        with serial.Serial('COM41',115200,timeout=.05) as port:
            try:
                raw+=read_available(port,.3)
                for command in ['off','p','i']:
                    send_line(port,command);raw+=read_available(port,.25)
                verify_off(raw)
                send_line(port,'cap1');ack=read_available(port,.25);raw+=ack
                if b'CAPTURE armed' not in ack: raise ValueError('capture not armed')
                send_line(port,'adcscancheck');raw+=read_available(port,1)
            finally:
                for command in ['off','cap0','stack','p','i']:
                    send_line(port,command);raw+=read_available(port,.25)
                args.out.write_bytes(raw)
        result=decode(raw.decode(errors='replace'))
    print('elapsed_us=',result['elapsed_us'],'scans=',len(result['scans']))
    print({k:(min(r[k] for r in result['scans']),max(r[k] for r in result['scans'])) for k in ['a','b','c','bus','vref']})
