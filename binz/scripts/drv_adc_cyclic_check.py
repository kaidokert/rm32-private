"""Driver-disabled cyclic DMA ownership probe and deliberate late-consumer refusal."""
import argparse
import base64
from pathlib import Path
import re
import struct
import zlib
from drv_capture import send_line,read_available,verify_off


def decode(text):
    heads=re.findall(r'^ADCCYCLIC (.*)$',text,re.M)
    if len(heads)!=1: raise ValueError('missing/duplicate cyclic header')
    h={k:int(v) for k,v in re.findall(r'(\w+)=(\d+)(?= |$)',heads[0])}
    if 'order=0,1,4,6,13' not in heads[0] or h.get('stopped')!=1 or h.get('gate_authority')!=0 or h.get('trigger_us')!=101:
        raise ValueError('unsafe/unsupported cyclic capture')
    if h.get('stall')==1:
        if (h.get('result'),h.get('lease_fault'),h.get('copied'))!=(9,2,0):
            raise ValueError('late consumer did not refuse ambiguous flags')
        if not 350<=h.get('elapsed_us',0)<1000: raise ValueError('stall timing')
    elif h.get('stall')==0:
        if (h.get('result'),h.get('lease_fault'),h.get('copied'))!=(0,0,32):
            raise ValueError('incomplete cyclic run')
        if not 3000<=h.get('elapsed_us',0)<=4000 or not 0<=h.get('copy_max_us',101)<=100:
            raise ValueError('copy/campaign timing')
    else: raise ValueError('missing stall mode')
    rows=[]
    for payload in re.findall(r'^CS85 (.*)$',text,re.M):
        raw=base64.a85decode(payload.strip())
        if len(raw)!=16 or zlib.crc32(raw[:12])!=int.from_bytes(raw[12:],'little'):
            raise ValueError('cyclic record CRC/length')
        w=struct.unpack('<6H',raw[:12])
        if w[0]!=len(rows) or max(w[1:])>4095 or not 0<w[5]<4095:
            raise ValueError('cyclic record sequence/range')
        rows.append(w)
    if len(rows)!=h['copied']: raise ValueError('cyclic count')
    verify_off(text[max(text.rfind('ADCCYCLIC'),text.rfind('\nCS85 ')):].encode())
    return h,rows


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path);ap.add_argument('--input',type=Path)
    ap.add_argument('--stall',action='store_true');args=ap.parse_args()
    if args.input: text=args.input.read_text()
    else:
        if args.out is None or args.out.exists(): ap.error('new --out required')
        import serial
        raw=b''
        with serial.Serial('COM41',115200,timeout=.05) as port:
            try:
                raw+=read_available(port,.3)
                for cmd in ['off','p','i']:
                    send_line(port,cmd);raw+=read_available(port,.25)
                verify_off(raw)
                send_line(port,'cap1');ack=read_available(port,.25);raw+=ack
                if b'CAPTURE armed' not in ack: raise ValueError('capture not armed')
                send_line(port,'adccyclicstall' if args.stall else 'adccycliccheck');raw+=read_available(port,1)
            finally:
                for cmd in ['off','cap0','stack','p','i']:
                    send_line(port,cmd);raw+=read_available(port,.25)
                args.out.write_bytes(raw)
        text=raw.decode(errors='replace')
    print(decode(text)[0])
